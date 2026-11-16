//! Veilleur de fichiers **sans dépendance** pour le rechargement à chaud (P2.4).
//!
//! Le Studio ne peut pas embarquer `notify` (crates.io bloqué, backends par
//! plateforme) : ce module interroge donc les dates de modification (`mtime`)
//! à la fréquence choisie par l'appelant. Une interrogation de quelques
//! dizaines de chemins ne coûte qu'un `stat` par fichier — négligeable à 800 ms.
//!
//! ## Sémantique
//!
//! - [`Veilleur::premier_passage`] photographie l'état courant **sans rien
//!   signaler** : il pose la référence à partir de laquelle les changements
//!   sont calculés.
//! - [`Veilleur::poll`] renvoie les chemins **nouveaux, modifiés ou supprimés**
//!   depuis la dernière interrogation, triés de façon déterministe. Un chemin
//!   inconnu (ajouté après le premier passage) est mémorisé sans être signalé :
//!   la première observation d'un fichier n'est pas un changement.
//! - Un fichier **absent au premier passage** n'est pas un changement ; s'il
//!   apparaît ensuite, son apparition l'est.
//! - `actif == false` suspend l'interrogation **sans perdre la référence** :
//!   les modifications survenues pendant la suspension sont signalées à la
//!   reprise. C'est ce qui évite de recharger pendant une saisie.
//!
//! Aucune I/O n'est faite hors des chemins fournis : le veilleur ne balaie
//! jamais un dossier. Toute erreur (`fichier disparu`, permission refusée,
//! mtime indisponible) devient [`None`] et n'interrompt jamais l'interrogation.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::game::GameState;

/// Date de modification d'un chemin, ou [`None`] s'il est absent/illisible.
///
/// Utiliser `metadata` (et non `symlink_metadata`) suit les liens symboliques :
/// un asset lié depuis le projet est surveillé à sa cible, qui est bien le
/// fichier modifié par l'outil de création.
fn mtime(chemin: &Path) -> Option<SystemTime> {
    fs::metadata(chemin)
        .ok()
        .and_then(|meta| meta.modified().ok())
}

/// Racine disque par défaut des chemins relatifs d'un état de jeu.
///
/// En développement et dans les tests, la racine du dépôt est la référence :
/// on la reconnaît à son dossier `assets/`. À défaut (binaire déplacé), le
/// répertoire courant reste un repli stable.
pub fn racine_par_defaut() -> PathBuf {
    let depot = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    if depot.join("assets").is_dir() {
        depot
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }
}

/// Résout un chemin authoré relativement à `racine`.
///
/// Un chemin relatif est **toujours** ancré sous `racine`, même si le fichier
/// a disparu : conserver le même chemin absolu est indispensable pour que la
/// suppression soit détectée (sinon la résolution changerait de cible entre
/// deux interrogations). Un chemin absolu est utilisé tel quel.
fn resoudre(chemin: &str, racine: &Path) -> PathBuf {
    let brut = Path::new(chemin);
    if brut.is_absolute() {
        brut.to_path_buf()
    } else {
        racine.join(brut)
    }
}

/// Veilleur par interrogation de `mtime`.
///
/// Les chemins sont triés et dédupliqués à la construction, ce qui rend l'ordre
/// des changements renvoyés indépendant de l'ordre d'insertion (donc testable).
pub struct Veilleur {
    /// Chemins surveillés, triés et sans doublon.
    chemins: Vec<PathBuf>,
    /// Dernière date observée par chemin ; `None` = fichier absent/illisible.
    vus: HashMap<PathBuf, Option<SystemTime>>,
    /// Si `false`, [`Veilleur::poll`] ne fait aucune I/O et renvoie une liste vide.
    actif: bool,
}

impl Veilleur {
    /// Construit un veilleur sur `chemins` (triés, dédupliqués, vides ignorés).
    ///
    /// Aucune I/O : appeler [`Veilleur::premier_passage`] avant la première
    /// interrogation pour poser la référence sans rien signaler.
    pub fn nouveau(mut chemins: Vec<PathBuf>) -> Self {
        chemins.retain(|chemin| !chemin.as_os_str().is_empty());
        chemins.sort();
        chemins.dedup();
        Self {
            chemins,
            vus: HashMap::new(),
            actif: true,
        }
    }

    /// Construit la liste des fichiers d'un état de jeu.
    ///
    /// Sont surveillés : la scène courante (`game.scene_path`), les scripts KVL
    /// référencés par les entités et les assets déclarés au manifeste (si le
    /// projet en porte un). Les chemins relatifs sont résolus sous
    /// [`racine_par_defaut`].
    pub fn pour_etat(etat: &GameState) -> Self {
        Self::pour_etat_depuis(etat, &racine_par_defaut())
    }

    /// Variante de [`Veilleur::pour_etat`] avec une racine explicite.
    ///
    /// Le moteur l'utilise pour les projets ouverts hors du dépôt : la racine
    /// est alors le dossier du fichier `.karacti`, et non celui du binaire.
    pub fn pour_etat_depuis(etat: &GameState, racine: &Path) -> Self {
        let mut chemins = Vec::new();
        if !etat.scene_path.trim().is_empty() {
            chemins.push(resoudre(&etat.scene_path, racine));
        }
        for entite in &etat.entities {
            if let Some(script) = &entite.script {
                if !script.script_path.trim().is_empty() {
                    chemins.push(resoudre(&script.script_path, racine));
                }
            }
        }
        if let Some(manifeste) = &etat.assets_manifest {
            for entree in &manifeste.entries {
                if !entree.path.trim().is_empty() {
                    chemins.push(resoudre(&entree.path, racine));
                }
            }
        }
        Self::nouveau(chemins)
    }

    /// Ajoute un chemin à surveiller (par exemple le fichier `.karacti`).
    ///
    /// L'ajout conserve le tri et l'unicité. Un chemin ajouté après le premier
    /// passage est découvert par le `poll` suivant **sans être signalé**.
    pub fn ajouter_chemin(&mut self, chemin: impl Into<PathBuf>) {
        let chemin = chemin.into();
        if chemin.as_os_str().is_empty() {
            return;
        }
        if self.chemins.binary_search(&chemin).is_ok() {
            return;
        }
        self.chemins.push(chemin);
        self.chemins.sort();
    }

    /// Mémorise l'état courant de tous les chemins **sans le signaler**.
    pub fn premier_passage(&mut self) {
        self.vus.clear();
        for chemin in &self.chemins {
            self.vus.insert(chemin.clone(), mtime(chemin));
        }
    }

    /// Active ou suspend l'interrogation, sans perdre la référence.
    pub fn changer_actif(&mut self, actif: bool) {
        self.actif = actif;
    }

    /// Renvoie les fichiers nouveaux, modifiés ou supprimés depuis le dernier
    /// `poll`, triés par chemin. Renvoie une liste vide si `actif == false`.
    pub fn poll(&mut self) -> Vec<PathBuf> {
        if !self.actif {
            return Vec::new();
        }

        let mut changes = Vec::new();
        for chemin in &self.chemins {
            let maintenant = mtime(chemin);
            match self.vus.get(chemin) {
                // Première observation (chemin ajouté après le passage) : on
                // mémorise sans signaler, comme un premier passage.
                None => {
                    self.vus.insert(chemin.clone(), maintenant);
                }
                Some(avant) => {
                    if *avant != maintenant {
                        changes.push(chemin.clone());
                        self.vus.insert(chemin.clone(), maintenant);
                    }
                }
            }
        }
        changes.sort();
        changes.dedup();
        changes
    }

    /// Chemins surveillés, triés (lecture seule).
    pub fn chemins(&self) -> &[PathBuf] {
        &self.chemins
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Dossier temporaire unique, supprimé automatiquement par `Drop`.
    struct Bac {
        dossier: PathBuf,
    }

    impl Bac {
        fn neuf(tag: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duree| duree.as_nanos())
                .unwrap_or(0);
            let dossier = std::env::temp_dir().join(format!(
                "karacti_veilleur_{tag}_{}_{nanos}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&dossier);
            fs::create_dir_all(&dossier).expect("dossier temporaire créé");
            Self { dossier }
        }

        fn ecrire(&self, nom: &str, octets: &[u8]) -> PathBuf {
            let chemin = self.dossier.join(nom);
            if let Some(parent) = chemin.parent() {
                fs::create_dir_all(parent).expect("sous-dossier temporaire créé");
            }
            fs::write(&chemin, octets).expect("fichier temporaire écrit");
            chemin
        }
    }

    impl Drop for Bac {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dossier);
        }
    }

    /// Force une référence « ancienne » pour un chemin, sans dormir : le test
    /// simule une modification de `mtime` de façon déterministe, même sur un
    /// système de fichiers à résolution grossière.
    fn vieillir(veilleur: &mut Veilleur, chemin: &Path) {
        veilleur
            .vus
            .insert(chemin.to_path_buf(), Some(UNIX_EPOCH));
    }

    #[test]
    fn premier_passage_ne_signale_rien() {
        let bac = Bac::neuf("premier");
        let chemin = bac.ecrire("scene.json", b"{}");

        let mut veilleur = Veilleur::nouveau(vec![chemin]);
        veilleur.premier_passage();

        assert!(veilleur.poll().is_empty(), "aucun changement après le premier passage");
        assert!(veilleur.poll().is_empty(), "toujours rien au passage suivant");
    }

    #[test]
    fn modification_de_mtime_signalee_une_seule_fois() {
        let bac = Bac::neuf("modification");
        let chemin = bac.ecrire("scene.json", b"v1");

        let mut veilleur = Veilleur::nouveau(vec![chemin.clone()]);
        veilleur.premier_passage();
        // La référence est artificiellement ancienne : la date réelle du fichier
        // diffère forcément, quel que soit le système de fichiers.
        vieillir(&mut veilleur, &chemin);

        assert_eq!(veilleur.poll(), vec![chemin.clone()], "le fichier modifié est signalé");
        assert!(veilleur.poll().is_empty(), "le changement n'est signalé qu'une fois");
    }

    #[test]
    fn suppression_signalee() {
        let bac = Bac::neuf("suppression");
        let chemin = bac.ecrire("asset.png", b"png");

        let mut veilleur = Veilleur::nouveau(vec![chemin.clone()]);
        veilleur.premier_passage();
        fs::remove_file(&chemin).expect("fichier supprimé");
        assert_eq!(veilleur.poll(), vec![chemin.clone()], "la suppression est un changement");
        assert!(veilleur.poll().is_empty(), "une seule notification de suppression");
    }

    #[test]
    fn aucun_changement_renvoie_une_liste_vide() {
        let bac = Bac::neuf("stable");
        let a = bac.ecrire("a.json", b"a");
        let b = bac.ecrire("b.json", b"b");

        let mut veilleur = Veilleur::nouveau(vec![a, b]);
        veilleur.premier_passage();
        assert!(veilleur.poll().is_empty());
        assert!(veilleur.poll().is_empty());
    }

    #[test]
    fn ordre_deterministe_malgre_un_ordre_d_insertion_inverse() {
        let bac = Bac::neuf("ordre");
        let b = bac.ecrire("b.json", b"b");
        let a = bac.ecrire("a.json", b"a");

        // Insertion volontairement inversée : le veilleur doit trier.
        let mut veilleur = Veilleur::nouveau(vec![b.clone(), a.clone()]);
        veilleur.premier_passage();
        vieillir(&mut veilleur, &a);
        vieillir(&mut veilleur, &b);

        let changes = veilleur.poll();
        assert_eq!(changes.len(), 2);
        let mut attendu = vec![a, b];
        attendu.sort();
        assert_eq!(changes, attendu, "l'ordre renvoyé doit être trié");
        assert_eq!(veilleur.chemins(), attendu.as_slice(), "les chemins sont triés");
    }

    #[test]
    fn chemin_absent_au_premier_passage_puis_cree_est_signale() {
        let bac = Bac::neuf("apparition");
        let chemin = bac.dossier.join("futur.json");

        let mut veilleur = Veilleur::nouveau(vec![chemin.clone()]);
        veilleur.premier_passage();
        assert!(veilleur.poll().is_empty(), "un chemin absent au départ n'est pas un changement");

        fs::write(&chemin, b"{}").expect("fichier créé");
        assert_eq!(veilleur.poll(), vec![chemin], "son apparition est un changement");
    }

    #[test]
    fn chemin_ajoute_apres_le_passage_n_est_pas_signale() {
        let bac = Bac::neuf("ajout");
        let a = bac.ecrire("a.json", b"a");
        let b = bac.ecrire("b.json", b"b");

        let mut veilleur = Veilleur::nouveau(vec![a]);
        veilleur.premier_passage();
        veilleur.ajouter_chemin(b);

        assert!(veilleur.poll().is_empty(), "découverte silencieuse du nouveau chemin");
    }

    #[test]
    fn suspension_ne_perd_pas_les_changements() {
        let bac = Bac::neuf("suspension");
        let chemin = bac.ecrire("scene.json", b"v1");

        let mut veilleur = Veilleur::nouveau(vec![chemin.clone()]);
        veilleur.premier_passage();
        veilleur.changer_actif(false);
        vieillir(&mut veilleur, &chemin);
        assert!(veilleur.poll().is_empty(), "inactif : aucune I/O, aucun signal");

        veilleur.changer_actif(true);
        assert_eq!(veilleur.poll(), vec![chemin], "le changement est signalé à la reprise");
    }

    #[test]
    fn pour_etat_collecte_scene_scripts_et_assets_sans_doublon() {
        use crate::assets_manifest::{
            AssetEntry, AssetKind, AssetsManifest, Provenance, ProvenanceSource,
        };

        let bac = Bac::neuf("etat");
        // Scène réelle : elle référence des scripts KVL par chemin relatif.
        let scene_path = bac.ecrire(
            "niveau.scene.json",
            include_str!("../../../assets/scenes/level_01.scene.json").as_bytes(),
        );
        let mut etat = crate::game::GameState::load_from_scene(&scene_path.display().to_string())
            .expect("état de jeu de test");
        assert!(
            etat.entities.iter().any(|entite| entite.script.is_some()),
            "la scène de test doit référencer au moins un script"
        );

        // Un asset réel + deux entrées pointant sur la scène (même chemin
        // résolu) : la déduplication doit garder une seule occurrence.
        let asset = bac.ecrire("assets/hero.png", b"png");
        let mut manifeste = AssetsManifest::default();
        let entree = |id: &str, path: &str, octets: &[u8]| AssetEntry {
            id: id.to_string(),
            kind: AssetKind::Sprite,
            path: path.to_string(),
            bytes: octets.len() as u64,
            hash: crate::ai_dictionary::hash_text(octets),
            provenance: Provenance {
                source: ProvenanceSource::Studio,
                license: Some("CC0-1.0".to_string()),
                created_at: "2026-11-16".to_string(),
            },
            dimensions: None,
            duration_ms: None,
        };
        manifeste
            .register(entree("hero", "assets/hero.png", b"png"))
            .expect("asset accepté");
        manifeste
            .register(entree("scene_bis", "niveau.scene.json", b"scene"))
            .expect("chemin de scène accepté");
        etat.assets_manifest = Some(manifeste);

        let veilleur = Veilleur::pour_etat_depuis(&etat, &bac.dossier);
        let chemins = veilleur.chemins();

        assert!(chemins.contains(&scene_path), "la scène est surveillée");
        assert!(chemins.contains(&asset), "l'asset du manifeste est surveillé");
        for script in etat
            .entities
            .iter()
            .filter_map(|entite| entite.script.as_ref())
        {
            assert!(
                chemins
                    .iter()
                    .any(|chemin| chemin.ends_with(Path::new(&script.script_path))),
                "le script « {} » doit être surveillé",
                script.script_path
            );
        }

        let mut uniques = chemins.to_vec();
        uniques.sort();
        uniques.dedup();
        assert_eq!(
            chemins.len(),
            uniques.len(),
            "les chemins surveillés doivent être dédupliqués"
        );
    }
}
