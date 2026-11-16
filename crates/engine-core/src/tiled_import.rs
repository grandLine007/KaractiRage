//! Import de cartes Tiled (`.tmx`, orthogonal) vers le format de scène du moteur.
//!
//! ## Choix de coordonnées
//!
//! Tiled et le moteur travaillent tous les deux en **y-down** avec une origine en
//! **haut-gauche**. Les coordonnées `x`/`y` des objets Tiled (en pixels) sont donc
//! recopiées telles quelles, sans retournement vertical, après ajout du décalage
//! éventuel de la couche d'objets (`offsetx`/`offsety`). La rotation Tiled, exprimée
//! en degrés horaires, est convertie en radians pour `Transform2D::rotation`.
//!
//! ## Limites v1 (assumées et signalées dans le rapport)
//!
//! - seules les cartes **orthogonales** sont acceptées (une autre orientation
//!   produit une erreur explicite) ; les cartes **infinies** voient leurs objets
//!   importés mais leurs couches de tuiles ignorées avec un avertissement ;
//! - chaque **couche de tuiles finie** devient **une entité `Tilemap`** (atlas
//!   PNG + indices de tuiles, voir [`ShapeKind::Tilemap`]) : les tuiles sont donc
//!   rendues comme dans Tiled, dans l'ordre des couches (z croissant, derrière
//!   les objets). Seul le **premier tileset** de la carte est importé ; son image
//!   est copiée dans `tilesets/<nom>.png` à côté de la carte et référencée par
//!   un chemin stable. Les tuiles des autres tilesets sont ignorées ;
//! - les **tuiles retournées/rotées** sont dessinées sans transformation et les
//!   **tuiles d'animation** sont figées sur leur première image (limites v1,
//!   signalées par avertissement) ;
//! - les **couches d'images**, les **groupes imbriqués** et les objets
//!   **polygones/polylignes/texte/capsule** sont ignorés avec un avertissement ;
//! - aucune dépendance nouvelle. La build vendorée de `tiled` n'active pas `zstd` :
//!   une carte compressée en zstd produit une erreur explicite, jamais un panic.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use thiserror::Error;
use tiled::{LayerType, Loader, ObjectShape, PropertyValue};

use crate::entity::{
    BossComponent, Collider2D, Entity, Health2D, KVLScriptComponent, Movement2D, Renderable2D,
    ShapeKind, Transform2D, VisionSensing,
};
use crate::project::KaractiProject;
use crate::recipe::GameGenre;
use crate::scene::SceneData;
use crate::subsystems::{UiElement, UiScreen, UiScreenKind};

/// Taille par défaut d'un objet rectangulaire Tiled sans `width`/`height`.
const TAILLE_OBJET_DEFAUT: f32 = 32.0;
/// Taille d'un objet « point » Tiled (marqueur de spawn discret).
const TAILLE_POINT: f32 = 16.0;
/// Taille d'un joueur issu d'un objet « point » (assez grand pour être jouable).
const TAILLE_JOUEUR_POINT: f32 = 32.0;
/// Vitesse du joueur importé (pixels/seconde).
const VITESSE_JOUEUR: f32 = 300.0;
/// Plafond de cellules d'une couche de tuiles importée (4096 × 4096) : au-delà,
/// le vecteur d'indices dépasserait 64 Mio et la carte est ignorée avec un
/// avertissement plutôt que de risquer une allocation démesurée.
const MAX_CELLULES_TILEMAP: usize = 4096 * 4096;

/// Rapport d'import d'une carte Tiled.
#[derive(Debug, Clone)]
pub struct TiledImportReport {
    /// Scène prête à être sérialisée puis chargée par `GameState`.
    pub scene: SceneData,
    /// Avertissements non bloquants (tilesets multiples, tuiles ignorées,
    /// couches non supportées, etc.).
    pub warnings: Vec<String>,
    /// Nombre total d'entités produites (objets + entités `Tilemap`).
    pub entities_imported: usize,
    /// Nombre de couches de tuiles converties en entité `Tilemap`.
    pub tile_layers_imported: usize,
    /// Ancien compteur de couches **aplaties**. Toujours `0` depuis que les
    /// tuiles sont importées : conservé pour ne pas casser l'IPC Studio.
    pub tile_layers_flattened: usize,
    /// Dimensions de la carte en **pixels** : `(largeur, hauteur)`.
    pub map_size: (u32, u32),
}

/// Résultat de la conversion d'une carte Tiled en projet KaractiRage sur disque.
///
/// Les deux fichiers sont écrits dans le même dossier : la scène
/// (`<nom>.scene.json`, relisible par `GameState::load_from_scene`) et le projet
/// (`<nom>.karacti`, relisible par `KaractiProject::load_from_file`).
#[derive(Debug, Clone)]
pub struct ImportedProject {
    /// Chemin du fichier `.karacti` écrit.
    pub project_path: PathBuf,
    /// Chemin du fichier `.scene.json` écrit.
    pub scene_path: PathBuf,
    /// Rapport d'import complet (scène, avertissements, statistiques).
    pub route: TiledImportReport,
}

/// Erreurs d'import d'une carte Tiled. Les messages sont destinés au créateur.
#[derive(Debug, Error)]
pub enum TiledImportError {
    /// Le fichier `.tmx` n'a pas pu être lu.
    #[error("Impossible de lire la carte Tiled « {chemin} » : {source}")]
    Lecture {
        /// Chemin demandé.
        chemin: String,
        /// Erreur système sous-jacente.
        #[source]
        source: std::io::Error,
    },

    /// La carte utilise la compression zstd, que la build vendorée ne gère pas.
    #[error(
        "Compression zstd non supportée dans cette build (crate `tiled` vendoré sans zstd) : \
         réexportez la carte en CSV ou zlib (Tiled : Fichier → Exporter → format TMX, \
         compression « CSV » ou « zlib »). Carte : {origin}"
    )]
    CompressionZstd {
        /// Origine de la carte (chemin ou libellé).
        origin: String,
    },

    /// Aucun objet de classe « player » n'a été trouvé.
    #[error(
        "La carte Tiled « {origin} » n'a pas d'objet joueur : ajoutez un objet de classe/type \
         « player » dans une couche d'objets (un rectangle par exemple)."
    )]
    JoueurAbsent {
        /// Origine de la carte (chemin ou libellé).
        origin: String,
    },

    /// Plusieurs objets joueur ont été trouvés.
    #[error(
        "La carte Tiled « {origin} » contient {count} objets joueur ; il en faut exactement un. \
         Gardez un seul objet de classe/type « player »."
    )]
    JoueursMultiples {
        /// Origine de la carte (chemin ou libellé).
        origin: String,
        /// Nombre d'objets joueur détectés.
        count: usize,
    },

    /// L'unique objet joueur utilise une forme non supportée en v1.
    #[error(
        "L'objet joueur de « {origin} » a une forme non supportée (polygone, polyligne, texte ou \
         capsule) : utilisez un rectangle, une ellipse ou un point."
    )]
    JoueurFormeNonSupportee {
        /// Origine de la carte (chemin ou libellé).
        origin: String,
    },

    /// Orientation de carte non orthogonale.
    #[error(
        "Orientation de carte « {orientation} » non supportée : l'import v1 accepte uniquement \
         les cartes orthogonales. Réexportez la carte en orientation orthogonale \
         (Propriétés de la carte → Orientation). Carte : {origin}"
    )]
    OrientationNonSupportee {
        /// Origine de la carte (chemin ou libellé).
        origin: String,
        /// Orientation lue dans le TMX (orthogonal, isometric, staggered, hexagonal).
        orientation: String,
    },

    /// Un tileset externe ou un template n'a pas pu être ouvert.
    #[error(
        "Ressource Tiled introuvable pour « {origin} » : « {chemin} » ({details}). Vérifiez que \
         les fichiers .tsx (tilesets externes) et .tx (templates) sont à côté de la carte ; \
         l'import v1 importe le premier tileset mais le parseur Tiled exige la présence de \
         toutes les ressources référencées."
    )]
    RessourceManquante {
        /// Origine de la carte (chemin ou libellé).
        origin: String,
        /// Chemin de la ressource manquante.
        chemin: String,
        /// Détail de l'erreur système.
        details: String,
    },

    /// Encodage/compression de couche non supporté (hors zstd, détecté plus tôt).
    #[error(
        "Encodage de couche non supporté dans « {origin} » (encoding={encoding}, \
         compression={compression}) : réexportez la carte au format CSV ou base64+zlib."
    )]
    EncodageNonSupporte {
        /// Origine de la carte (chemin ou libellé).
        origin: String,
        /// Valeur de l'attribut `encoding`.
        encoding: String,
        /// Valeur de l'attribut `compression`.
        compression: String,
    },

    /// Le TMX est mal formé ou utilise une fonctionnalité non gérée par `tiled`.
    #[error("La carte Tiled « {origin} » est illisible : {details}")]
    CarteInvalide {
        /// Origine de la carte (chemin ou libellé).
        origin: String,
        /// Détail fourni par le parseur.
        details: String,
    },

    /// Le dossier de destination du projet n'a pas pu être préparé.
    #[error(
        "Impossible de préparer le dossier de destination « {chemin} » : {details}. \
         Choisissez un dossier accessible en écriture."
    )]
    DossierDestination {
        /// Dossier demandé.
        chemin: String,
        /// Détail de l'erreur système.
        details: String,
    },

    /// Un fichier de sortie (scène ou projet) n'a pas pu être écrit.
    #[error(
        "Impossible d'écrire « {chemin} » : {details}. Vérifiez les droits d'écriture du dossier \
         et l'espace disque disponible."
    )]
    EcritureFichier {
        /// Fichier de sortie concerné.
        chemin: String,
        /// Détail de l'erreur (système ou sérialisation).
        details: String,
    },
}

/// Importe une carte Tiled depuis le disque.
///
/// Les tilesets externes et templates éventuels sont lus relativement au fichier
/// `.tmx` (comportement du crate `tiled`). L'image du **premier** tileset est
/// copiée dans `tilesets/` à côté de la carte, puis référencée par les entités
/// `Tilemap` de la scène produite.
pub fn import_tmx(path: &Path) -> Result<TiledImportReport, TiledImportError> {
    let xml = std::fs::read_to_string(path).map_err(|source| TiledImportError::Lecture {
        chemin: path.display().to_string(),
        source,
    })?;
    let origin = path.display().to_string();
    protege_du_panic(&origin, || {
        import_impl(&compacter_csv(&xml), &origin, Some(path))
    })
}

/// Importe une carte Tiled **et l'écrit comme projet KaractiRage prêt à ouvrir**.
///
/// La carte est convertie en scène, puis deux fichiers sont écrits dans
/// `dest_dir` (par défaut : le dossier de la carte `.tmx`) :
///
/// - `<nom>.scene.json` : la scène, chargeable seule par `GameState::load_from_scene` ;
/// - `<nom>.karacti` : un `KaractiProject` complet (métadonnées issues du nom de la
///   carte, une seule scène, écrans UI par défaut, graphe racine KVL), chargeable
///   par `KaractiProject::load_from_file` puis `GameState::load_project`.
///
/// `<nom>` est le nom de la carte (nom de fichier sans extension), assaini pour
/// rester un nom de fichier portable. Les assets du dépôt ne sont jamais
/// modifiés : la seule écriture hors sortie est la copie
/// `tilesets/<nom-tileset>.png` **à côté de la carte**, faite par
/// [`import_tmx`], qui rend la carte autonome une fois déplacée avec son dossier
/// `tilesets/`.
pub fn import_tmx_to_project(
    tmx_path: &Path,
    dest_dir: Option<&Path>,
) -> Result<ImportedProject, TiledImportError> {
    let route = import_tmx(tmx_path)?;
    let titre = route.scene.name.clone();
    let nom_fichier = nom_fichier_sur(&titre);
    let dossier = dest_dir
        .map(Path::to_path_buf)
        .or_else(|| tmx_path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));

    std::fs::create_dir_all(&dossier).map_err(|erreur| TiledImportError::DossierDestination {
        chemin: dossier.display().to_string(),
        details: erreur.to_string(),
    })?;

    // 1. La scène seule d'abord : même si la construction du projet échouait, le
    // créateur garde un `.scene.json` utilisable.
    let scene_path = dossier.join(format!("{nom_fichier}.scene.json"));
    let scene_json = serde_json::to_string_pretty(&route.scene).map_err(|erreur| {
        TiledImportError::EcritureFichier {
            chemin: scene_path.display().to_string(),
            details: erreur.to_string(),
        }
    })?;
    std::fs::write(&scene_path, scene_json).map_err(|erreur| {
        TiledImportError::EcritureFichier {
            chemin: scene_path.display().to_string(),
            details: erreur.to_string(),
        }
    })?;

    // 2. Le projet complet : `new_empty` fournit le graphe racine KVL et des
    // réglages cohérents ; la scène générée remplace celle du projet vierge.
    let mut projet = KaractiProject::new_empty(&titre, "KaractiRage Studio", GameGenre::Adventure);
    projet.metadata.description = format!(
        "Projet importé depuis la carte Tiled « {} ». Scène prête à être ouverte dans \
         l'Atelier Studio.",
        tmx_path.display()
    );
    projet.metadata.dimension = Some("2D".to_string());
    projet.settings.default_mode = "edit".to_string();
    projet.scene = route.scene.clone();
    projet.scenes = vec![route.scene.clone()];
    // Écrans par défaut (HUD + Pause) : un projet importé est immédiatement jouable
    // et personnalisable, sans écran de menu qui masquerait la carte au lancement.
    projet.ui_screens = ecrans_par_defaut();

    let project_path = dossier.join(format!("{nom_fichier}.karacti"));
    projet
        .save_to_file(&project_path)
        .map_err(|erreur| TiledImportError::EcritureFichier {
            chemin: project_path.display().to_string(),
            details: erreur.to_string(),
        })?;

    Ok(ImportedProject {
        project_path,
        scene_path,
        route,
    })
}

/// Assainit un nom de carte pour en faire un nom de fichier portable.
///
/// Les lettres/chiffres Unicode, `-` et `_` sont conservés ; tout le reste devient
/// un `_` (les suites de séparateurs sont compactées). Un nom vide devient
/// `carte_tiled`.
fn nom_fichier_sur(nom: &str) -> String {
    let mut sortie = String::with_capacity(nom.len());
    let mut separateur_precedent = false;
    for caractere in nom.trim().chars() {
        if caractere.is_alphanumeric() || caractere == '-' || caractere == '_' {
            sortie.push(caractere);
            separateur_precedent = false;
        } else if !separateur_precedent {
            sortie.push('_');
            separateur_precedent = true;
        }
    }
    let nettoye = sortie.trim_matches('_').to_string();
    if nettoye.is_empty() {
        "carte_tiled".to_string()
    } else {
        nettoye
    }
}

/// Écrans UI par défaut d'un projet importé : HUD (vie + score) et Pause.
///
/// Aucun écran `Menu` n'est déclaré : le moteur démarre donc directement sur la
/// carte importée au lieu d'afficher un menu d'accueil.
fn ecrans_par_defaut() -> Vec<UiScreen> {
    vec![
        UiScreen {
            id: "hud_jeu".to_string(),
            kind: UiScreenKind::Hud,
            title: None,
            elements: vec![
                UiElement {
                    id: "vie".to_string(),
                    kind: "bar".to_string(),
                    label: Some("Vie".to_string()),
                    bind: Some("player_hp".to_string()),
                },
                UiElement {
                    id: "score".to_string(),
                    kind: "text".to_string(),
                    label: Some("Score".to_string()),
                    bind: Some("score".to_string()),
                },
            ],
        },
        UiScreen {
            id: "pause".to_string(),
            kind: UiScreenKind::Pause,
            title: Some("Pause".to_string()),
            elements: vec![UiElement {
                id: "reprendre".to_string(),
                kind: "button".to_string(),
                label: Some("Reprendre".to_string()),
                bind: None,
            }],
        },
    ]
}

/// Importe une carte Tiled depuis une chaîne XML (tests sans disque).
///
/// `origin` sert uniquement aux messages d'erreur et au nom de la scène. Les
/// éventuelles ressources externes sont résolues relativement au répertoire
/// d'`origin` (ou au répertoire courant si `origin` n'a pas de parent).
/// Compacte les blocs `<data encoding="csv">` avant de les donner au parseur.
///
/// Tiled exporte une ligne par rangée de la carte. Le crate `tiled` vendoré ne
/// tolère pas les sauts de ligne dans les données CSV : une carte de plus d'une
/// rangée échouait avec « invalid digit found in string ». On joint donc les
/// valeurs en retirant tout espace, ce qui ne change ni l'ordre ni les valeurs.
fn compacter_csv(xml: &str) -> String {
    let mut sortie = String::with_capacity(xml.len());
    let mut reste = xml;
    while let Some(debut) = reste.find("<data") {
        sortie.push_str(&reste[..debut]);
        let bloc = &reste[debut..];
        let Some(fin) = bloc.find("</data>") else {
            sortie.push_str(bloc);
            return sortie;
        };
        let entete_fin = match bloc.find('>') {
            Some(index) => index + 1,
            None => {
                sortie.push_str(bloc);
                return sortie;
            }
        };
        let entete = &bloc[..entete_fin];
        if entete.contains("encoding=\"csv\"") {
            let contenu = &bloc[entete_fin..fin];
            // Les rangées sont séparées par des sauts de ligne : on joint par des
            // virgules (retirer les espaces collerait « …4 » et « 1… » en « 41 »).
            let compacte: String = contenu
                .split(|c: char| c == ',' || c.is_whitespace())
                .filter(|valeur| !valeur.is_empty())
                .collect::<Vec<_>>()
                .join(",");
            sortie.push_str(entete);
            sortie.push_str(&compacte);
            sortie.push_str("</data>");
        } else {
            sortie.push_str(&bloc[..fin + "</data>".len()]);
        }
        reste = &bloc[fin + "</data>".len()..];
    }
    sortie.push_str(reste);
    sortie
}

/// Exécute l'import en convertissant tout panic du parseur vendoré en erreur.
///
/// Le crate `tiled` panique (`index out of bounds`) sur une carte dont les données
/// ne couvrent pas `largeur × hauteur`. Une entrée utilisateur ne doit jamais faire
/// tomber le processus : on renvoie une `CarteInvalide` actionnable.
fn protege_du_panic<F>(origin: &str, import: F) -> Result<TiledImportReport, TiledImportError>
where
    F: FnOnce() -> Result<TiledImportReport, TiledImportError>,
{
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(import)) {
        Ok(resultat) => resultat,
        Err(_) => Err(TiledImportError::CarteInvalide {
            origin: origin.to_string(),
            details: "carte incohérente : le nombre de tuiles diffère de largeur × hauteur \
                      (ou les données sont illisibles)".to_string(),
        }),
    }
}

pub fn import_tmx_str(xml: &str, origin: &str) -> Result<TiledImportReport, TiledImportError> {
    protege_du_panic(origin, || import_impl(&compacter_csv(xml), origin, None))
}

fn import_impl(
    xml: &str,
    origin: &str,
    chemin: Option<&Path>,
) -> Result<TiledImportReport, TiledImportError> {
    // Détection faite AVANT le parseur : le crate vendoré sans zstd renverrait
    // sinon une erreur générique « encoding/compression inconnus ».
    if contient_compression_zstd(xml) {
        return Err(TiledImportError::CompressionZstd {
            origin: origin.to_string(),
        });
    }

    let map_path = chemin
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(origin));
    let map_bytes = xml.as_bytes().to_vec();
    let map_path_lecteur = map_path.clone();

    // Lecteur en mémoire pour la carte ; les tilesets/templates externes sont lus
    // sur disque par le crate, relativement au chemin de la carte.
    let mut loader =
        Loader::with_reader(move |demande: &Path| -> std::io::Result<Cursor<Vec<u8>>> {
            if demande == map_path_lecteur {
                Ok(Cursor::new(map_bytes.clone()))
            } else {
                std::fs::read(demande).map(Cursor::new)
            }
        });

    let map = loader
        .load_tmx_map(&map_path)
        .map_err(|erreur| convertir_erreur_tiled(erreur, origin))?;

    if map.orientation != tiled::Orientation::Orthogonal {
        return Err(TiledImportError::OrientationNonSupportee {
            origin: origin.to_string(),
            orientation: format!("{:?}", map.orientation),
        });
    }

    let mut warnings = Vec::new();
    let mut entites: Vec<Entity> = Vec::new();
    let mut prochain_id = 1usize;
    let mut couches_tuiles_importees = 0usize;
    let total_couches = map.layers().count();

    // Un joueur et un seul : vérifié avant de construire quoi que ce soit pour
    // produire l'erreur la plus directe possible.
    let nombre_joueurs = map
        .layers()
        .filter_map(|couche| couche.as_object_layer())
        .flat_map(|couche| couche.objects())
        .filter(|objet| tag_depuis_classe(&objet.user_type) == "player")
        .count();
    if nombre_joueurs == 0 {
        return Err(TiledImportError::JoueurAbsent {
            origin: origin.to_string(),
        });
    }
    if nombre_joueurs > 1 {
        return Err(TiledImportError::JoueursMultiples {
            origin: origin.to_string(),
            count: nombre_joueurs,
        });
    }

    if map.infinite() {
        warnings.push(
            "Carte Tiled infinie : les couches de tuiles ne sont pas importées (les objets sont \
             importés tels quels). Utilisez une carte finie pour obtenir un décor."
                .to_string(),
        );
    }

    if map.tilesets().len() > 1 {
        let premier = map
            .tilesets()
            .first()
            .map(|tileset| tileset.name.as_str())
            .filter(|nom| !nom.trim().is_empty())
            .unwrap_or("sans nom");
        warnings.push(format!(
            "Carte « {origin} » : {} tilesets détectés ; seul le premier « {premier} » est importé \
             (limite v1). Les tuiles des autres tilesets sont ignorées.",
            map.tilesets().len()
        ));
    }

    // Le premier tileset est préparé une seule fois : copie de l'image à côté de
    // la carte et métadonnées de découpe partagées par toutes les couches.
    let tileset_importe = preparer_tileset(
        map.tilesets().first().map(|tileset| tileset.as_ref()),
        chemin,
        origin,
        &mut warnings,
    );
    if let Some(ref infos) = tileset_importe {
        if infos.tile_w != map.tile_width || infos.tile_h != map.tile_height {
            warnings.push(format!(
                "Tileset « {} » : tuiles de {}×{} px dans une carte à {}×{} px ; elles sont \
                 rendues à leur taille native, sans étirement (limite v1).",
                infos.nom, infos.tile_w, infos.tile_h, map.tile_width, map.tile_height
            ));
        }
    }

    for (index_couche, couche) in map.layers().enumerate() {
        match couche.layer_type() {
            LayerType::Objects(couche_objets) => {
                // Le décalage de couche est appliqué aux objets, comme dans Tiled.
                let decalage_x = couche.offset_x;
                let decalage_y = couche.offset_y;
                for objet in couche_objets.objects() {
                    importer_objet(
                        &objet,
                        index_couche,
                        decalage_x,
                        decalage_y,
                        &mut prochain_id,
                        &mut entites,
                        &mut warnings,
                    );
                }
            }
            LayerType::Tiles(couche_tuiles) => {
                let (Some(largeur_tuiles), Some(hauteur_tuiles)) =
                    (couche_tuiles.width(), couche_tuiles.height())
                else {
                    // Carte infinie : l'avertissement global a déjà été émis.
                    if !map.infinite() {
                        warnings.push(format!(
                            "Couche de tuiles « {} » ignorée : couche infinie non supportée en v1.",
                            couche.name
                        ));
                    }
                    continue;
                };

                if largeur_tuiles == 0 || hauteur_tuiles == 0 {
                    warnings.push(format!(
                        "Couche de tuiles « {} » ignorée : dimensions nulles.",
                        couche.name
                    ));
                    continue;
                }

                let nom_couche = if couche.name.trim().is_empty() {
                    format!("couche_{}", index_couche)
                } else {
                    couche.name.clone()
                };

                let Some(ref infos) = tileset_importe else {
                    warnings.push(format!(
                        "Couche de tuiles « {} » ignorée : aucun tileset importable dans la carte \
                         (seul le premier tileset avec une image est utilisé en v1).",
                        nom_couche
                    ));
                    continue;
                };

                let Some(nombre_cellules) =
                    (largeur_tuiles as usize).checked_mul(hauteur_tuiles as usize)
                else {
                    warnings.push(format!(
                        "Couche de tuiles « {nom_couche} » ignorée : dimensions trop grandes."
                    ));
                    continue;
                };
                if nombre_cellules > MAX_CELLULES_TILEMAP {
                    warnings.push(format!(
                        "Couche de tuiles « {nom_couche} » ignorée : {nombre_cellules} cellules \
                         dépassent le plafond de {MAX_CELLULES_TILEMAP} (limite v1)."
                    ));
                    continue;
                }
                let mut tuiles = vec![0u32; nombre_cellules];
                let mut retournees = 0usize;
                let mut animees = 0usize;
                let mut autres_tilesets = 0usize;

                for y in 0..hauteur_tuiles {
                    for x in 0..largeur_tuiles {
                        let Some(tuile) = couche_tuiles.get_tile(x as i32, y as i32) else {
                            continue; // cellule vide
                        };
                        // `tileset_index` est relatif à l'ordre de la carte : seul
                        // l'index 0 (premier tileset) est importé en v1.
                        if tuile.tileset_index() != 0 {
                            autres_tilesets += 1;
                            continue;
                        }
                        if tuile.flip_h || tuile.flip_v || tuile.flip_d {
                            retournees += 1;
                        }
                        // Tuile d'animation : on fige la première image (v1).
                        let mut index_local = tuile.id();
                        if let Some(definition) = tuile.get_tile() {
                            if let Some(animation) = &definition.animation {
                                if let Some(premiere) = animation.first() {
                                    index_local = premiere.tile_id;
                                    animees += 1;
                                }
                            }
                        }
                        // `0` = vide, donc l'indice local (0-based) est décalé de 1.
                        tuiles[y as usize * largeur_tuiles as usize + x as usize] =
                            index_local.saturating_add(1);
                    }
                }

                if autres_tilesets > 0 {
                    warnings.push(format!(
                        "Couche de tuiles « {} » : {autres_tilesets} tuile(s) d'un autre tileset \
                         ignorée(s) (seul le premier tileset est importé).",
                        nom_couche
                    ));
                }
                if retournees > 0 {
                    warnings.push(format!(
                        "Couche de tuiles « {} » : {retournees} tuile(s) retournée(s)/rotée(s) \
                         dessinée(s) sans transformation (limite v1).",
                        nom_couche
                    ));
                }
                if animees > 0 {
                    warnings.push(format!(
                        "Couche de tuiles « {} » : {animees} tuile(s) animée(s) figée(s) sur leur \
                         première image (limite v1).",
                        nom_couche
                    ));
                }

                if tuiles.iter().all(|&tuile| tuile == 0) {
                    // Couche vide (ou uniquement composée de tuiles d'autres
                    // tilesets) : aucune entité à créer, comme dans Tiled.
                    continue;
                }

                // Une tuile est rendue à sa taille native ; la grille de l'entité
                // fait donc exactement `colonnes × tile_w` par `lignes × tile_h`.
                let largeur = largeur_tuiles as f32 * infos.tile_w as f32;
                let hauteur = hauteur_tuiles as f32 * infos.tile_h as f32;
                // Les couches de tuiles restent derrière les objets ; l'ordre des
                // couches Tiled est conservé (une couche plus basse passe dessus).
                let z_index = index_couche as i32 - total_couches as i32;
                let rendu = Renderable2D::tilemap(
                    infos.asset.clone(),
                    infos.tile_w,
                    infos.tile_h,
                    infos.columns,
                    tuiles,
                    z_index,
                );

                let entite = Entity::new(
                    prochain_id,
                    format!("tiled:{}", nom_couche),
                    "tilemap",
                    Transform2D::new(couche.offset_x, couche.offset_y, largeur, hauteur),
                )
                .with_renderable(rendu);

                entites.push(entite);
                prochain_id += 1;
                couches_tuiles_importees += 1;
            }
            LayerType::Image(_) => warnings.push(format!(
                "Couche d'image « {} » ignorée : l'import v1 ne gère pas les images de fond.",
                couche.name
            )),
            LayerType::Group(_) => warnings.push(format!(
                "Groupe de couches « {} » ignoré : les groupes imbriqués ne sont pas supportés \
                 en v1. Placez les objets dans une couche de premier niveau.",
                couche.name
            )),
        }
    }

    if !entites.iter().any(|entite| entite.tag == "player") {
        return Err(TiledImportError::JoueurFormeNonSupportee {
            origin: origin.to_string(),
        });
    }

    let nom_carte = Path::new(origin)
        .file_stem()
        .map(|nom| nom.to_string_lossy().trim().to_string())
        .filter(|nom| !nom.is_empty())
        .unwrap_or_else(|| "Carte Tiled".to_string());

    let scene = SceneData {
        name: nom_carte,
        description: format!(
            "Importée depuis « {origin} » par l'importeur Tiled v1 (couches d'objets + couches \
             de tuiles Tilemap)."
        ),
        objective: None,
        entities: entites,
        player: None,
        coins: Vec::new(),
        doors: Vec::new(),
        walls: Vec::new(),
        enemies: Vec::new(),
    };

    Ok(TiledImportReport {
        entities_imported: scene.entities.len(),
        scene,
        warnings,
        tile_layers_imported: couches_tuiles_importees,
        // Plus aucune couche n'est aplatie : le champ reste à 0 pour l'IPC Studio.
        tile_layers_flattened: 0,
        map_size: (
            map.width.saturating_mul(map.tile_width),
            map.height.saturating_mul(map.tile_height),
        ),
    })
}

/// Tileset prêt à être référencé par les entités `Tilemap`.
struct TilesetImporte {
    /// Nom lisible du tileset (messages d'avertissement).
    nom: String,
    /// Identifiant stable de l'atlas (chemin absolu du PNG copié).
    asset: String,
    tile_w: u32,
    tile_h: u32,
    columns: u32,
}

/// Prépare le premier tileset : valide ses dimensions, copie son image dans
/// `tilesets/<nom>.png` à côté de la carte et renvoie les métadonnées de découpe.
///
/// `carte` vaut `None` pour un import en mémoire ([`import_tmx_str`]) : aucune
/// copie n'est alors tentée, l'image d'origine est référencée telle quelle.
fn preparer_tileset(
    tileset: Option<&tiled::Tileset>,
    carte: Option<&Path>,
    origin: &str,
    warnings: &mut Vec<String>,
) -> Option<TilesetImporte> {
    let Some(tileset) = tileset else {
        return None;
    };
    let nom = nom_tileset(tileset, Path::new("tileset"));
    if tileset.tile_width == 0 || tileset.tile_height == 0 {
        warnings.push(format!(
            "Tileset « {nom} » : dimensions de tuile invalides ({}×{}), il est ignoré.",
            tileset.tile_width, tileset.tile_height,
        ));
        return None;
    }
    let Some(image) = &tileset.image else {
        warnings.push(format!(
            "Tileset « {nom} » : collection d'images (pas d'atlas unique) non supportée en v1 ; \
             les couches de tuiles sont ignorées."
        ));
        return None;
    };
    let columns = if tileset.columns > 0 {
        tileset.columns
    } else if image.width > 0 {
        (image.width as u32 / tileset.tile_width).max(1)
    } else {
        1
    };
    let asset = match carte {
        Some(chemin) => copier_image_tileset(chemin, &nom, &image.source, origin, warnings),
        None => image.source.display().to_string(),
    };
    Some(TilesetImporte {
        nom,
        asset,
        tile_w: tileset.tile_width,
        tile_h: tileset.tile_height,
        columns,
    })
}

/// Nom de fichier portable du tileset (`tuiles`, `couleurs`, …).
fn nom_tileset(tileset: &tiled::Tileset, source_par_defaut: &Path) -> String {
    let brut = if !tileset.name.trim().is_empty() {
        tileset.name.trim().to_string()
    } else {
        source_par_defaut
            .file_stem()
            .map(|nom| nom.to_string_lossy().to_string())
            .filter(|nom| !nom.trim().is_empty())
            .unwrap_or_else(|| "tileset".to_string())
    };
    nom_fichier_sur(&brut)
}

/// Copie l'image d'un tileset dans `tilesets/` à côté de la carte et renvoie le
/// chemin **absolu** du fichier copié (identifiant stable, résolvable quel que
/// soit le répertoire courant du moteur).
///
/// En cas d'image introuvable ou de copie impossible, un avertissement est
/// émis et l'image d'origine est référencée : l'import continue, le rendu
/// retombera sur les rectangles de repli et le pré-vol d'export signalera
/// l'asset manquant.
fn copier_image_tileset(
    carte: &Path,
    nom: &str,
    source: &Path,
    origin: &str,
    warnings: &mut Vec<String>,
) -> String {
    if !source.is_file() {
        warnings.push(format!(
            "Tileset « {nom} » de « {origin} » : image « {} » introuvable ; la carte sera \
             importée avec un rendu de repli.",
            source.display()
        ));
        return chemin_absolu(source);
    }

    let dossier_carte = carte
        .parent()
        .filter(|dossier| !dossier.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let dossier_tilesets = dossier_carte.join("tilesets");
    let destination = dossier_tilesets.join(format!("{nom}.png"));

    let deja_place = match (source.canonicalize(), destination.canonicalize()) {
        (Ok(source), Ok(destination)) => source == destination,
        _ => source == destination,
    };
    if !deja_place {
        let resultat = std::fs::create_dir_all(&dossier_tilesets)
            .and_then(|_| std::fs::copy(source, &destination).map(|_| ()));
        if let Err(erreur) = resultat {
            warnings.push(format!(
                "Tileset « {nom} » : impossible de copier « {} » vers « {} » ({erreur}) ; la \
                 carte référence l'image d'origine.",
                source.display(),
                destination.display()
            ));
            return chemin_absolu(source);
        }
    }
    chemin_absolu(&destination)
}

/// Chemin absolu si possible, sinon chemin tel quel (jamais de panic).
fn chemin_absolu(chemin: &Path) -> String {
    chemin
        .canonicalize()
        .unwrap_or_else(|_| chemin.to_path_buf())
        .display()
        .to_string()
}

/// Convertit un objet Tiled en entité moteur et l'ajoute à la scène.
fn importer_objet(
    objet: &tiled::Object<'_>,
    index_couche: usize,
    decalage_x: f32,
    decalage_y: f32,
    prochain_id: &mut usize,
    entites: &mut Vec<Entity>,
    warnings: &mut Vec<String>,
) {
    let tag = tag_depuis_classe(&objet.user_type);
    let nom_brut = if objet.name.trim().is_empty() {
        tag.to_string()
    } else {
        objet.name.clone()
    };

    let Some(forme) = forme_objet(&objet.shape, tag) else {
        warnings.push(format!(
            "Objet « {} » de la couche #{} ignoré : forme non supportée en v1 (rectangles, \
             ellipses et points uniquement).",
            nom_brut, index_couche
        ));
        return;
    };

    let (largeur, hauteur, shape) = match &forme {
        Forme::Rect { w, h } => (*w, *h, ShapeKind::Rect),
        Forme::Circle { radius } => (
            radius * 2.0,
            radius * 2.0,
            ShapeKind::Circle { radius: *radius },
        ),
        Forme::Point { size } => (*size, *size, ShapeKind::Rect),
    };

    let id = *prochain_id;
    *prochain_id += 1;

    let nom = nom_objet(objet, id, tag);
    let mut transform =
        Transform2D::new(objet.x + decalage_x, objet.y + decalage_y, largeur, hauteur);
    transform.rotation = objet.rotation.to_radians();

    let (couleur, contour, z_index) = apparence(tag);
    let mut rendu = Renderable2D {
        shape,
        color: couleur,
        outline_color: contour,
        outline_width: if contour.is_some() { 2 } else { 0 },
        z_index,
        visible: objet.visible,
    };

    if let Some(valeur) = objet.properties.get("color") {
        match couleur_depuis_propriete(valeur) {
            Some(couleur_personnalisee) => rendu.color = couleur_personnalisee,
            None => warnings.push(format!(
                "Propriété « color » invalide sur l'objet « {} » (attendu #RRGGBB) : couleur par \
                 défaut conservée.",
                nom
            )),
        }
    }

    let mut entite = Entity::new(id, nom.clone(), tag, transform).with_renderable(rendu);

    // Collider : la propriété `solid` explicite est prioritaire sur le défaut du tag.
    match objet.properties.get("solid").and_then(lire_bool) {
        Some(true) => {
            entite = entite.with_collider(Collider2D::solid(largeur, hauteur));
        }
        Some(false) => {}
        None => match tag {
            "wall" | "door" | "player" | "boss" => {
                entite = entite.with_collider(Collider2D::solid(largeur, hauteur));
            }
            "pickup" | "switch" | "teleport" => {
                entite = entite.with_collider(Collider2D::trigger(largeur, hauteur));
            }
            _ => {}
        },
    }

    // Santé : propriété `health` (ou alias `hp`), sinon valeurs par défaut
    // raisonnables pour le joueur et le boss.
    let sante = objet
        .properties
        .get("health")
        .or_else(|| objet.properties.get("hp"))
        .and_then(lire_nombre);
    if let Some(max) = sante {
        entite = entite.with_health(Health2D::new(max.max(0.0)));
    } else if tag == "player" {
        entite = entite.with_health(Health2D::new(3.0));
    } else if tag == "boss" {
        entite = entite.with_health(Health2D::new(20.0));
    }

    // Script KVL fourni par la propriété `script`.
    if let Some(script_path) = objet.properties.get("script").and_then(lire_chaine) {
        if !script_path.trim().is_empty() {
            let mut script = KVLScriptComponent::new(script_path);
            match tag {
                "door" => script.on_interact_event = Some("OnDoorInteract".to_string()),
                "pickup" => script.on_overlap_event = Some("OnPlayerOverlap".to_string()),
                "enemy" => script.on_seen_event = Some("OnPlayerSeen".to_string()),
                "boss" => {
                    script.on_overlap_event = Some("OnBossOverlap".to_string());
                    script.on_seen_event = Some("OnBossPhaseChange".to_string());
                }
                _ => {}
            }
            entite = entite.with_script(script);
        }
    }

    // Comportements par défaut, pour qu'une carte Tiled soit directement jouable.
    let y = objet.y + decalage_y;
    match tag {
        "player" => {
            entite = entite.with_movement(Movement2D::player(VITESSE_JOUEUR));
        }
        "enemy" => {
            entite = entite.with_movement(Movement2D::patrol_vertical(
                100.0,
                (y - 100.0).max(0.0),
                y + 100.0,
            ));
            entite = entite.with_vision(VisionSensing::new(200.0));
        }
        "boss" => {
            entite = entite.with_boss(BossComponent::new(nom, "GARDIEN ANCESTRAL"));
        }
        _ => {}
    }

    entites.push(entite);
}

/// Forme géométrique normalisée d'un objet Tiled.
enum Forme {
    Rect { w: f32, h: f32 },
    Circle { radius: f32 },
    Point { size: f32 },
}

/// Traduit une `class`/`type` Tiled en tag moteur.
///
/// Correspondances : `player`, `wall`/`collision`, `door`, `enemy`,
/// `item`/`pickup`, `boss`, `switch`, `teleport`. Toute autre valeur devient
/// `decor` (import sans comportement).
fn tag_depuis_classe(classe: &str) -> &'static str {
    match classe.trim().to_ascii_lowercase().as_str() {
        "player" => "player",
        "wall" | "collision" => "wall",
        "door" => "door",
        "enemy" => "enemy",
        "item" | "pickup" => "pickup",
        "boss" => "boss",
        "switch" => "switch",
        "teleport" => "teleport",
        _ => "decor",
    }
}

/// Normalise la forme d'un objet Tiled. `None` = forme ignorée en v1.
///
/// - rectangle : `width`/`height` tels quels, 32 px si absents ou nuls ;
/// - ellipse : approchée par un cercle de diamètre `min(width, height)` (16 px de
///   rayon si dimensions absentes) ; une ellipse très allongée perd son aspect ;
/// - point : petite entité 16 × 16, sauf le joueur (32 × 32) pour rester jouable.
fn forme_objet(shape: &ObjectShape, tag: &str) -> Option<Forme> {
    match shape {
        ObjectShape::Rect { width, height } => {
            let w = if *width > 0.0 {
                *width
            } else {
                TAILLE_OBJET_DEFAUT
            };
            let h = if *height > 0.0 {
                *height
            } else {
                TAILLE_OBJET_DEFAUT
            };
            Some(Forme::Rect { w, h })
        }
        ObjectShape::Ellipse { width, height } => {
            let diametre = width.min(*height);
            let diametre = if diametre > 0.0 {
                diametre
            } else {
                TAILLE_OBJET_DEFAUT
            };
            Some(Forme::Circle {
                radius: diametre * 0.5,
            })
        }
        ObjectShape::Point(..) => {
            let size = if tag == "player" {
                TAILLE_JOUEUR_POINT
            } else {
                TAILLE_POINT
            };
            Some(Forme::Point { size })
        }
        // Polygones, polylignes, texte et capsules : non supportés en v1.
        _ => None,
    }
}

/// Couleur, contour et `z_index` par défaut d'un tag moteur.
fn apparence(tag: &str) -> (u32, Option<u32>, i32) {
    match tag {
        "player" => (0x0038BDF8, Some(0x00FFFFFF), 10),
        "wall" => (0x001E293B, Some(0x00475569), 1),
        "door" => (0x00EF4444, Some(0x00FCA5A5), 2),
        "enemy" => (0x00DC2626, Some(0x00F87171), 8),
        "pickup" => (0x00F59E0B, Some(0x00FDE047), 5),
        "boss" => (0x0006B6D4, Some(0x0038BDF8), 9),
        "switch" => (0x00A855F7, Some(0x00E9D5FF), 2),
        "teleport" => (0x0006B6D4, Some(0x0067E8F9), 3),
        _ => (0x00334155, Some(0x00475569), 0),
    }
}

/// Nom final : propriété `name`, puis nom d'objet Tiled, puis `<tag>_<id>`.
fn nom_objet(objet: &tiled::Object<'_>, id: usize, tag: &str) -> String {
    if let Some(nom) = objet.properties.get("name").and_then(lire_chaine) {
        if !nom.trim().is_empty() {
            return nom;
        }
    }
    if !objet.name.trim().is_empty() {
        return objet.name.clone();
    }
    format!("{}_{}", tag, id)
}

/// Lit une propriété Tiled comme chaîne (`string` ou `file`).
fn lire_chaine(valeur: &PropertyValue) -> Option<String> {
    match valeur {
        PropertyValue::StringValue(texte) | PropertyValue::FileValue(texte) => Some(texte.clone()),
        _ => None,
    }
}

/// Lit une propriété Tiled comme booléen (bool, int ou chaîne usuelle).
fn lire_bool(valeur: &PropertyValue) -> Option<bool> {
    match valeur {
        PropertyValue::BoolValue(b) => Some(*b),
        PropertyValue::IntValue(i) => Some(*i != 0),
        PropertyValue::StringValue(texte) => match texte.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "oui" => Some(true),
            "false" | "0" | "non" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

/// Lit une propriété Tiled comme nombre (float, int ou chaîne numérique).
fn lire_nombre(valeur: &PropertyValue) -> Option<f32> {
    match valeur {
        PropertyValue::FloatValue(f) => Some(*f),
        PropertyValue::IntValue(i) => Some(*i as f32),
        PropertyValue::StringValue(texte) => texte.trim().parse::<f32>().ok(),
        _ => None,
    }
}

/// Lit une couleur Tiled (`color` typée ou chaîne `#RRGGBB`/`#AARRGGBB`).
///
/// Le moteur stocke ses couleurs en `0x00RRGGBB` : l'éventuel canal alpha Tiled
/// (`#AARRGGBB`) est ignoré.
fn couleur_depuis_propriete(valeur: &PropertyValue) -> Option<u32> {
    match valeur {
        PropertyValue::ColorValue(couleur) => Some(
            ((couleur.red as u32) << 16) | ((couleur.green as u32) << 8) | (couleur.blue as u32),
        ),
        PropertyValue::StringValue(texte) => analyser_couleur(texte),
        _ => None,
    }
}

/// Analyse `#RRGGBB` ou `#AARRGGBB` en `0x00RRGGBB`.
fn analyser_couleur(texte: &str) -> Option<u32> {
    let texte = texte.trim();
    let chiffres = texte.strip_prefix('#').unwrap_or(texte);
    let rgb = match chiffres.len() {
        6 => chiffres,
        8 => &chiffres[2..],
        _ => return None,
    };
    u32::from_str_radix(rgb, 16)
        .ok()
        .map(|valeur| valeur & 0x00FF_FFFF)
}

/// Détecte `compression="zstd"` dans un élément `<data>` du TMX.
///
/// La détection est faite sur le XML brut car le crate `tiled` vendoré sans zstd
/// ne renvoie qu'une erreur générique d'encodage inconnu.
fn contient_compression_zstd(xml: &str) -> bool {
    let minuscules = xml.to_ascii_lowercase();
    let mut offset = 0usize;
    while let Some(position) = minuscules[offset..].find("<data") {
        let debut = offset + position;
        let fin = minuscules[debut..]
            .find('>')
            .map(|position| debut + position)
            .unwrap_or(minuscules.len());
        if attribut_egale(&minuscules[debut..fin], "compression", "zstd") {
            return true;
        }
        offset = fin.saturating_add(1);
        if offset >= minuscules.len() {
            break;
        }
    }
    false
}

/// Vrai si l'attribut `nom` vaut `valeur` (comparaison insensible à la casse).
fn attribut_egale(balise: &str, nom: &str, valeur: &str) -> bool {
    let mut reste = balise;
    while let Some(position) = reste.find(nom) {
        let apres = &reste[position + nom.len()..];
        reste = apres;
        let apres = apres.trim_start();
        let Some(apres) = apres.strip_prefix('=') else {
            continue;
        };
        let apres = apres.trim_start();
        let Some(guillemet) = apres.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            continue;
        };
        let contenu = &apres[1..];
        let valeur_lue = contenu.split(guillemet).next().unwrap_or("");
        if valeur_lue.trim().eq_ignore_ascii_case(valeur) {
            return true;
        }
    }
    false
}

/// Traduit une erreur du crate `tiled` en erreur actionnable pour le créateur.
fn convertir_erreur_tiled(erreur: tiled::Error, origin: &str) -> TiledImportError {
    match erreur {
        tiled::Error::InvalidEncodingFormat {
            encoding,
            compression,
        } => {
            let compression_lue = compression.unwrap_or_else(|| "aucune".to_string());
            if compression_lue.eq_ignore_ascii_case("zstd") {
                TiledImportError::CompressionZstd {
                    origin: origin.to_string(),
                }
            } else {
                TiledImportError::EncodageNonSupporte {
                    origin: origin.to_string(),
                    encoding: encoding.unwrap_or_else(|| "aucun".to_string()),
                    compression: compression_lue,
                }
            }
        }
        tiled::Error::ResourceLoadingError { path, err } => TiledImportError::RessourceManquante {
            origin: origin.to_string(),
            chemin: path.display().to_string(),
            details: err.to_string(),
        },
        autre => TiledImportError::CarteInvalide {
            origin: origin.to_string(),
            details: autre.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::ShapeKind;
    use crate::exporter::StandaloneExporter;
    use crate::game::GameState;
    use crate::renderer::Canvas;
    use std::path::PathBuf;

    fn fixture(nom: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(nom)
    }

    /// Copie des fixtures dans un dossier temporaire et renvoie le chemin de la
    /// carte (premier fichier). Les imports Tiled écrivent la copie du tileset à
    /// côté de la carte : les fixtures du dépôt doivent rester intactes.
    fn carte_temporaire(suffixe: &str, fichiers: &[&str]) -> PathBuf {
        let dossier = dossier_temporaire_unique(suffixe);
        for fichier in fichiers {
            std::fs::copy(fixture(fichier), dossier.join(fichier))
                .unwrap_or_else(|erreur| panic!("copie de {fichier} : {erreur}"));
        }
        dossier.join(fichiers[0])
    }

    #[test]
    fn test_a_carte_minimale_joueur_murs_porte() {
        let rapport = import_tmx(&fixture("minimal.tmx")).expect("carte minimale importable");
        assert!(rapport.warnings.is_empty(), "{:?}", rapport.warnings);
        assert_eq!(rapport.entities_imported, 4);
        assert_eq!(rapport.scene.entities.len(), 4);
        assert_eq!(rapport.map_size, (800, 608));
        assert_eq!(rapport.tile_layers_flattened, 0);
        assert_eq!(rapport.tile_layers_imported, 0);

        let tags: Vec<&str> = rapport
            .scene
            .entities
            .iter()
            .map(|entite| entite.tag.as_str())
            .collect();
        assert_eq!(tags, ["player", "wall", "wall", "door"]);

        let joueur = &rapport.scene.entities[0];
        assert_eq!(joueur.name, "Joueur");
        assert_eq!(
            (
                joueur.transform.x,
                joueur.transform.y,
                joueur.transform.w,
                joueur.transform.h
            ),
            (64.0, 96.0, 32.0, 32.0)
        );
        assert!(
            joueur
                .movement
                .as_ref()
                .expect("mouvement joueur")
                .is_player_controlled
        );
        assert!(joueur.collider.as_ref().expect("collider joueur").is_solid);

        let mur_nord = &rapport.scene.entities[1];
        assert_eq!(
            (
                mur_nord.transform.x,
                mur_nord.transform.y,
                mur_nord.transform.w,
                mur_nord.transform.h
            ),
            (200.0, 96.0, 64.0, 32.0)
        );
        let collider = mur_nord.collider.as_ref().expect("collider mur");
        assert!(collider.is_solid);
        assert_eq!((collider.w, collider.h), (64.0, 32.0));

        let mur_est = &rapport.scene.entities[2];
        assert_eq!((mur_est.transform.w, mur_est.transform.h), (32.0, 96.0));

        let porte = &rapport.scene.entities[3];
        assert_eq!(porte.name, "Porte");
        assert!(porte.collider.as_ref().expect("collider porte").is_solid);
        assert_eq!((porte.transform.w, porte.transform.h), (32.0, 64.0));
    }

    #[test]
    fn test_b_proprietes_personnalisees_appliquees() {
        let rapport = import_tmx(&fixture("proprietes.tmx")).expect("carte propriétés");
        assert_eq!(rapport.entities_imported, 3);

        let mur = rapport
            .scene
            .entities
            .iter()
            .find(|entite| entite.name == "Mur Mystère")
            .expect("propriété name appliquée");
        assert_eq!(mur.tag, "wall");
        assert_eq!(
            mur.script.as_ref().expect("script").script_path,
            "assets/scripts/door_interaction.kvl.json"
        );
        assert!(mur.collider.as_ref().expect("collider").is_solid);
        assert_eq!(mur.health.as_ref().expect("health").max, 5.0);
        assert_eq!(mur.renderable.as_ref().expect("rendu").color, 0x00FF_00AA);

        let fantome = rapport
            .scene
            .entities
            .iter()
            .find(|entite| entite.name == "MurFantome")
            .expect("second mur");
        assert!(
            fantome.collider.is_none(),
            "solid=false doit retirer le collider par défaut"
        );
        assert_eq!(fantome.health.as_ref().expect("hp").max, 7.0);
        assert_eq!(
            fantome.renderable.as_ref().expect("rendu").color,
            0x0012_3456
        );
    }

    #[test]
    fn test_c_carte_sans_joueur_erreur_claire() {
        let erreur = import_tmx(&fixture("sans_joueur.tmx")).expect_err("joueur manquant");
        assert!(matches!(erreur, TiledImportError::JoueurAbsent { .. }));
        let message = erreur.to_string();
        assert!(message.contains("joueur"), "message : {message}");
        assert!(message.contains("player"), "message : {message}");
    }

    #[test]
    fn test_d_couche_de_tuiles_devient_une_entite_tilemap() {
        let carte = carte_temporaire(
            "couche_tuiles",
            &["couche_tuiles.tmx", "tuiles.png"],
        );
        let rapport = import_tmx(&carte).expect("carte tuiles");
        assert_eq!(rapport.tile_layers_imported, 1);
        assert_eq!(rapport.tile_layers_flattened, 0, "plus aucune couche aplatie");
        assert_eq!(rapport.entities_imported, 2);

        // Le tileset fait 16×16 px alors que la carte est en 32×32 : les tuiles
        // sont rendues à leur taille native, avec un avertissement explicite.
        assert!(
            rapport
                .warnings
                .iter()
                .any(|avertissement| avertissement.contains("taille native")),
            "avertissements : {:?}",
            rapport.warnings
        );

        let tilemap = rapport
            .scene
            .entities
            .iter()
            .find(|entite| entite.tag == "tilemap")
            .expect("entité Tilemap");
        assert_eq!(tilemap.name, "tiled:Sol");
        assert_eq!(
            (
                tilemap.transform.x,
                tilemap.transform.y,
                tilemap.transform.w,
                tilemap.transform.h
            ),
            (0.0, 0.0, 64.0, 48.0),
            "4 × 3 cellules de 16×16 px"
        );

        match &tilemap.renderable.as_ref().expect("rendu").shape {
            ShapeKind::Tilemap {
                asset,
                tile_w,
                tile_h,
                columns,
                tiles,
            } => {
                assert_eq!((*tile_w, *tile_h, *columns), (16, 16, 1));
                assert_eq!(tiles.len(), 12);
                assert!(tiles.iter().all(|&tuile| tuile == 1), "{tiles:?}");
                let copie = Path::new(asset.as_str());
                assert!(copie.is_file(), "atlas copié : {asset}");
                assert_eq!(copie.file_name().and_then(|nom| nom.to_str()), Some("tuiles.png"));
                assert_eq!(
                    std::fs::read(copie).expect("lecture copie"),
                    std::fs::read(fixture("tuiles.png")).expect("lecture source")
                );
            }
            autre => panic!("forme Tilemap attendue, obtenu {autre:?}"),
        }

        let _ = std::fs::remove_dir_all(carte.parent().expect("dossier temporaire"));
    }

    #[test]
    fn test_e_compression_zstd_erreur_explicite() {
        let erreur = import_tmx(&fixture("zstd.tmx")).expect_err("zstd non supporté");
        assert!(matches!(erreur, TiledImportError::CompressionZstd { .. }));
        let message = erreur.to_string().to_ascii_lowercase();
        assert!(message.contains("zstd"), "message : {message}");
        assert!(message.contains("csv"), "message : {message}");
        assert!(message.contains("zlib"), "message : {message}");
    }

    #[test]
    fn test_f_scene_importee_chargeable_par_le_moteur() {
        let rapport = import_tmx(&fixture("minimal.tmx")).expect("carte minimale");
        let attendu = rapport.entities_imported;
        let chemin = std::env::temp_dir().join(format!(
            "karactirage_tiled_{}.scene.json",
            std::process::id()
        ));
        let json = serde_json::to_string_pretty(&rapport.scene).expect("sérialisation scène");
        std::fs::write(&chemin, json).expect("écriture scène temporaire");

        let etat = GameState::load_from_scene(chemin.to_str().expect("chemin UTF-8"))
            .expect("scène chargeable depuis le JSON");
        assert_eq!(etat.entities.len(), attendu);
        assert!(etat.player_index().is_some(), "joueur contrôlable présent");
        let _ = std::fs::remove_file(&chemin);

        let etat_direct = GameState::from_scene_data(rapport.scene, "test_tiled.tmx")
            .expect("scène chargeable en mémoire");
        assert_eq!(etat_direct.entities.len(), attendu);
        assert!(etat_direct.player_index().is_some());
    }

    #[test]
    fn test_g_ellipse_et_point() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<map version="1.10" orientation="orthogonal" width="10" height="10" tilewidth="32" tileheight="32" infinite="0">
 <objectgroup name="Spawns">
  <object id="1" class="player" x="10" y="20"><point/></object>
  <object id="2" class="item" x="100" y="50" width="20" height="20"><ellipse/></object>
  <object id="3" class="enemy" x="200" y="70"><point/></object>
 </objectgroup>
</map>"#;
        let rapport = import_tmx_str(xml, "inline.tmx").expect("import en mémoire");
        assert_eq!(rapport.scene.entities.len(), 3);

        let joueur = &rapport.scene.entities[0];
        assert_eq!((joueur.transform.w, joueur.transform.h), (32.0, 32.0));
        assert_eq!((joueur.transform.x, joueur.transform.y), (10.0, 20.0));

        let piece = &rapport.scene.entities[1];
        assert_eq!(piece.tag, "pickup");
        assert_eq!(
            piece.renderable.as_ref().expect("rendu").shape,
            ShapeKind::Circle { radius: 10.0 }
        );
        assert_eq!((piece.transform.w, piece.transform.h), (20.0, 20.0));
        assert!(piece.collider.as_ref().expect("collider").is_trigger);

        let ennemi = &rapport.scene.entities[2];
        assert_eq!(ennemi.tag, "enemy");
        assert_eq!((ennemi.transform.w, ennemi.transform.h), (16.0, 16.0));
        assert!(ennemi.vision.is_some());
    }

    /// Dossier temporaire unique par test (le nom évite les collisions entre tests
    /// exécutés en parallèle dans le même processus).
    fn dossier_temporaire_unique(suffixe: &str) -> PathBuf {
        let dossier = std::env::temp_dir().join(format!(
            "karactirage_tiled_{suffixe}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dossier);
        std::fs::create_dir_all(&dossier).expect("création du dossier temporaire");
        dossier
    }

    #[test]
    fn test_h_import_tmx_to_project_ecrit_projet_et_scene_relus() {
        let dossier = dossier_temporaire_unique("projet");
        let carte = dossier.join("carte_atelier.tmx");
        std::fs::copy(fixture("minimal.tmx"), &carte).expect("copie de la carte de test");

        let importe = import_tmx_to_project(&carte, None).expect("import en projet");
        assert_eq!(
            importe.project_path.parent(),
            Some(dossier.as_path()),
            "par défaut, le projet est écrit à côté de la carte"
        );
        assert_eq!(
            importe
                .project_path
                .file_name()
                .and_then(|nom| nom.to_str()),
            Some("carte_atelier.karacti")
        );
        assert_eq!(
            importe.scene_path.file_name().and_then(|nom| nom.to_str()),
            Some("carte_atelier.scene.json")
        );
        assert!(importe.project_path.is_file(), "fichier .karacti écrit");
        assert!(importe.scene_path.is_file(), "fichier .scene.json écrit");

        // Le projet est relisible par l'API publique du moteur.
        let projet = KaractiProject::load_from_file(&importe.project_path).expect("projet relu");
        assert_eq!(projet.metadata.title, "carte_atelier");
        assert_eq!(projet.scenes.len(), 1, "un projet importé n'a qu'une scène");
        assert_eq!(
            projet.scene.entities.len(),
            importe.route.entities_imported,
            "la scène du projet est la carte importée"
        );
        assert!(
            !projet.ui_screens.is_empty(),
            "des écrans UI par défaut sont fournis"
        );
        assert_eq!(importe.route.entities_imported, 4);

        // La scène seule est chargeable par le moteur, avec son joueur contrôlable.
        let etat = GameState::load_from_scene(
            importe
                .scene_path
                .to_str()
                .expect("chemin de scène en UTF-8"),
        )
        .expect("scène relue par le moteur");
        assert_eq!(etat.entities.len(), importe.route.entities_imported);
        assert!(etat.player_index().is_some(), "joueur contrôlable présent");

        // Aucun fichier n'a été créé dans les assets du dépôt.
        let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        assert!(!assets.join("carte_atelier.karacti").exists());
        assert!(!assets.join("carte_atelier.scene.json").exists());

        let _ = std::fs::remove_dir_all(&dossier);
    }

    #[test]
    fn test_i_import_tmx_to_project_destination_personnalisee() {
        let dossier = dossier_temporaire_unique("destination");
        let carte = dossier.join("carte_dest.tmx");
        std::fs::copy(fixture("couche_tuiles.tmx"), &carte).expect("copie de la carte de test");
        std::fs::copy(fixture("tuiles.png"), dossier.join("tuiles.png"))
            .expect("copie du tileset de test");
        let destination = dossier.join("projets_crees");

        let importe = import_tmx_to_project(&carte, Some(&destination))
            .expect("import vers une destination explicite");
        assert_eq!(importe.project_path.parent(), Some(destination.as_path()));
        assert_eq!(
            importe
                .project_path
                .file_name()
                .and_then(|nom| nom.to_str()),
            Some("carte_dest.karacti")
        );
        assert_eq!(
            importe.scene_path.file_name().and_then(|nom| nom.to_str()),
            Some("carte_dest.scene.json")
        );
        assert_eq!(importe.route.tile_layers_imported, 1);
        assert_eq!(importe.route.tile_layers_flattened, 0);

        let projet = KaractiProject::load_from_file(&importe.project_path).expect("projet relu");
        assert_eq!(projet.scene.entities.len(), importe.route.entities_imported);
        assert_eq!(projet.metadata.title, "carte_dest");
        assert!(
            projet
                .scene
                .entities
                .iter()
                .any(|entite| entite.tag == "tilemap"),
            "la couche de tuiles est bien un Tilemap dans le projet"
        );

        let _ = std::fs::remove_dir_all(&dossier);
    }

    /// Entité `Tilemap` unique d'un rapport d'import.
    fn tilemap_de(rapport: &TiledImportReport) -> &Entity {
        rapport
            .scene
            .entities
            .iter()
            .find(|entite| entite.tag == "tilemap")
            .expect("une entité Tilemap attendue")
    }

    /// Importe la carte, construit l'état et rend une frame dans un canvas
    /// 400×300 avec la caméra calée en (0, 0).
    fn rendre_carte(carte: &Path) -> Canvas {
        let rapport = import_tmx(carte).expect("import de la carte");
        let mut etat = GameState::from_scene_data(
            rapport.scene.clone(),
            carte.to_str().expect("chemin UTF-8"),
        )
        .expect("état chargeable");
        etat.editor.embedded_mode = true;
        etat.editor.mode = crate::editor::EditorMode::Play;
        etat.camera.x = 0.0;
        etat.camera.y = 0.0;
        let mut canvas = Canvas::new(400, 300);
        etat.render(&mut canvas);
        canvas
    }

    #[test]
    fn test_j_import_tilemap_une_entite_par_couche_et_atlas_copie() {
        let carte = carte_temporaire(
            "tilemap_couleurs",
            &["tilemap_couleurs.tmx", "tilesets_petit.png"],
        );
        let rapport = import_tmx(&carte).expect("carte couleurs");
        assert_eq!(rapport.tile_layers_imported, 1);
        assert_eq!(rapport.tile_layers_flattened, 0);
        assert_eq!(rapport.entities_imported, 2, "joueur + tuiles");
        assert!(
            rapport.warnings.is_empty(),
            "aucun avertissement attendu : {:?}",
            rapport.warnings
        );

        let tilemap = tilemap_de(&rapport);
        assert_eq!(tilemap.name, "tiled:Sol");
        assert_eq!(
            (
                tilemap.transform.x,
                tilemap.transform.y,
                tilemap.transform.w,
                tilemap.transform.h
            ),
            (100.0, 120.0, 4.0, 2.0),
            "2 × 1 cellules de 2×2 px, décalage de couche conservé"
        );

        match &tilemap.renderable.as_ref().expect("rendu").shape {
            ShapeKind::Tilemap {
                asset,
                tile_w,
                tile_h,
                columns,
                tiles,
            } => {
                assert_eq!((*tile_w, *tile_h, *columns), (2, 2, 2));
                assert_eq!(tiles, &vec![1, 2], "indices locaux décalés de 1 (0 = vide)");
                let copie = Path::new(asset.as_str());
                assert!(copie.is_file(), "atlas copié à côté de la carte : {asset}");
                assert_eq!(
                    copie.file_name().and_then(|nom| nom.to_str()),
                    Some("couleurs.png")
                );
                assert_eq!(
                    copie
                        .parent()
                        .and_then(Path::file_name)
                        .and_then(|nom| nom.to_str()),
                    Some("tilesets")
                );
                assert_eq!(
                    std::fs::read(copie).expect("lecture de la copie"),
                    std::fs::read(fixture("tilesets_petit.png")).expect("lecture de la source")
                );
            }
            autre => panic!("forme Tilemap attendue, obtenu {autre:?}"),
        }

        // Sérialisation JSON : le variant `Tilemap` doit survivre à un aller-retour
        // de scène (format lu par le Studio et par l'export).
        let json = serde_json::to_string(&rapport.scene).expect("sérialisation de la scène");
        let relu: SceneData = serde_json::from_str(&json).expect("scène relue");
        assert!(
            relu.entities.iter().any(|entite| {
                matches!(
                    entite.renderable.as_ref().map(|rendu| &rendu.shape),
                    Some(ShapeKind::Tilemap { .. })
                )
            }),
            "variant Tilemap désérialisé"
        );

        let _ = std::fs::remove_dir_all(carte.parent().expect("dossier temporaire"));
    }

    #[test]
    fn test_k_tileset_externe_tsx_importe() {
        let carte = carte_temporaire(
            "tilemap_externe",
            &[
                "tilemap_externe.tmx",
                "tileset_externe.tsx",
                "tilesets_petit.png",
            ],
        );
        let rapport = import_tmx(&carte).expect("carte à tileset externe");
        assert_eq!(rapport.tile_layers_imported, 1);

        let tilemap = tilemap_de(&rapport);
        match &tilemap.renderable.as_ref().expect("rendu").shape {
            ShapeKind::Tilemap {
                asset,
                tile_w,
                tile_h,
                columns,
                tiles,
            } => {
                assert_eq!((*tile_w, *tile_h, *columns), (2, 2, 2));
                assert_eq!(tiles, &vec![1, 2]);
                assert_eq!(
                    Path::new(asset.as_str())
                        .file_name()
                        .and_then(|nom| nom.to_str()),
                    Some("externe.png"),
                    "nom repris du .tsx externe"
                );
                assert!(Path::new(asset.as_str()).is_file());
            }
            autre => panic!("forme Tilemap attendue, obtenu {autre:?}"),
        }

        let _ = std::fs::remove_dir_all(carte.parent().expect("dossier temporaire"));
    }

    #[test]
    fn test_l_tuiles_animees_et_retournees_signalees() {
        let carte = carte_temporaire(
            "tilemap_animee",
            &["tilemap_animee.tmx", "tilesets_petit.png"],
        );
        let rapport = import_tmx(&carte).expect("carte animée");
        assert_eq!(rapport.tile_layers_imported, 1);
        assert!(
            rapport
                .warnings
                .iter()
                .any(|avertissement| avertissement.contains("animée")),
            "avertissements : {:?}",
            rapport.warnings
        );
        assert!(
            rapport
                .warnings
                .iter()
                .any(|avertissement| avertissement.contains("retournée")),
            "avertissements : {:?}",
            rapport.warnings
        );

        match &tilemap_de(&rapport).renderable.as_ref().expect("rendu").shape {
            ShapeKind::Tilemap { tiles, .. } => {
                // Les deux cellules référencent la tuile 0, qui est animée :
                // chacune est figée sur la tuile 1 de l'animation → index 2.
                // Sans la résolution de l'animation, on lirait l'index 1.
                assert_eq!(tiles, &vec![2, 2]);
            }
            autre => panic!("forme Tilemap attendue, obtenu {autre:?}"),
        }

        let _ = std::fs::remove_dir_all(carte.parent().expect("dossier temporaire"));
    }

    #[test]
    fn test_m_carte_sans_tileset_couche_vide_sans_tilemap() {
        let carte = carte_temporaire("tilemap_vide", &["tilemap_vide.tmx"]);
        let rapport = import_tmx(&carte).expect("carte sans tileset");
        assert_eq!(rapport.tile_layers_imported, 0);
        assert_eq!(rapport.tile_layers_flattened, 0);
        assert_eq!(rapport.entities_imported, 1, "seul le joueur est importé");
        assert!(
            rapport
                .scene
                .entities
                .iter()
                .all(|entite| entite.tag != "tilemap"),
            "aucune entité Tilemap pour une couche vide"
        );
        assert!(
            rapport
                .warnings
                .iter()
                .any(|avertissement| avertissement.contains("tileset")),
            "avertissements : {:?}",
            rapport.warnings
        );

        let _ = std::fs::remove_dir_all(carte.parent().expect("dossier temporaire"));
    }

    #[test]
    fn test_n_plusieurs_tilesets_avertissement_et_premier_utilise() {
        let carte = carte_temporaire(
            "tilemap_multi",
            &["tilemap_multi.tmx", "tilesets_petit.png", "tuiles.png"],
        );
        let rapport = import_tmx(&carte).expect("carte multi-tilesets");
        assert_eq!(rapport.tile_layers_imported, 1);
        assert!(
            rapport
                .warnings
                .iter()
                .any(|avertissement| avertissement.contains("2 tilesets")
                    && avertissement.contains("premier")),
            "avertissements : {:?}",
            rapport.warnings
        );
        assert!(
            rapport
                .warnings
                .iter()
                .any(|avertissement| avertissement.contains("autre tileset")),
            "avertissements : {:?}",
            rapport.warnings
        );

        match &tilemap_de(&rapport).renderable.as_ref().expect("rendu").shape {
            ShapeKind::Tilemap { tiles, columns, .. } => {
                assert_eq!(*columns, 2, "colonnes du premier tileset");
                assert_eq!(tiles, &vec![1, 0], "la tuile du 2e tileset est ignorée");
            }
            autre => panic!("forme Tilemap attendue, obtenu {autre:?}"),
        }

        let _ = std::fs::remove_dir_all(carte.parent().expect("dossier temporaire"));
    }

    #[test]
    fn test_o_rendu_des_tuiles_aux_couleurs_de_l_atlas() {
        let carte = carte_temporaire(
            "tilemap_rendu",
            &["tilemap_couleurs.tmx", "tilesets_petit.png"],
        );
        let canvas = rendre_carte(&carte);
        let pixel = |x: usize, y: usize| canvas.buffer[y * canvas.width + x];
        // Couche posée à (100, 120) : tuile 0 rouge en (100..102, 120..122),
        // tuile 1 bleue en (102..104, 120..122). Le blitter opaque écrit la
        // couleur sans canal alpha (`0x00RRGGBB`).
        assert_eq!(pixel(101, 121), 0x00FF_0000, "tuile rouge de l'atlas");
        assert_eq!(pixel(103, 121), 0x0000_00FF, "tuile bleue de l'atlas");

        let _ = std::fs::remove_dir_all(carte.parent().expect("dossier temporaire"));
    }

    #[test]
    fn test_p_export_embarque_le_tileset_et_le_paquet_rend_la_carte() {
        let carte = carte_temporaire(
            "tilemap_export",
            &["tilemap_couleurs.tmx", "tilesets_petit.png"],
        );
        let rapport = import_tmx(&carte).expect("carte couleurs");
        let etat = GameState::from_scene_data(
            rapport.scene.clone(),
            carte.to_str().expect("chemin UTF-8"),
        )
        .expect("état source");

        let asset = match &tilemap_de(&rapport).renderable.as_ref().expect("rendu").shape {
            ShapeKind::Tilemap { asset, .. } => asset.clone(),
            autre => panic!("forme Tilemap attendue, obtenu {autre:?}"),
        };

        let paquet = StandaloneExporter::create_package(&etat, "Tiled", "test");
        assert!(
            paquet.sprite_assets.contains_key(&asset),
            "l'atlas doit voyager dans sprite_assets : {:?}",
            paquet.sprite_assets.keys().collect::<Vec<_>>()
        );
        assert!(!paquet.sprite_assets[&asset].is_empty());

        // L'atlas devient illisible sur disque : seul l'asset embarqué du
        // paquet peut encore alimenter le rendu.
        std::fs::remove_file(&asset).expect("suppression de l'atlas copié");

        let mut cible = GameState::from_scene_data(
            SceneData {
                name: "cible".to_string(),
                description: String::new(),
                objective: None,
                entities: Vec::new(),
                player: None,
                coins: Vec::new(),
                doors: Vec::new(),
                walls: Vec::new(),
                enemies: Vec::new(),
            },
            "cible.scene.json",
        )
        .expect("état cible vide");
        StandaloneExporter::import_package(&mut cible, &paquet);
        cible.screen = crate::screens::GameScreen::Playing;
        cible.editor.embedded_mode = true;
        cible.editor.mode = crate::editor::EditorMode::Play;
        cible.camera.x = 0.0;
        cible.camera.y = 0.0;
        let mut canvas = Canvas::new(400, 300);
        cible.render(&mut canvas);

        let pixel = |x: usize, y: usize| canvas.buffer[y * canvas.width + x];
        assert_eq!(pixel(101, 121), 0x00FF_0000, "rouge depuis l'asset embarqué");
        assert_eq!(pixel(103, 121), 0x0000_00FF, "bleu depuis l'asset embarqué");
        let couleurs: std::collections::HashSet<u32> = canvas
            .buffer
            .iter()
            .copied()
            .filter(|pixel| *pixel != 0x0008_0B14)
            .collect();
        assert!(couleurs.len() > 2, "la carte ne doit pas être uniforme");

        let _ = std::fs::remove_dir_all(carte.parent().expect("dossier temporaire"));
    }

    #[test]
    fn test_r_deux_couches_deux_entites_et_ordre_z() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<map version="1.10" orientation="orthogonal" width="2" height="1" tilewidth="2" tileheight="2" infinite="0">
 <tileset firstgid="1" name="couleurs" tilewidth="2" tileheight="2" tilecount="2" columns="2">
  <image source="tilesets_petit.png" width="4" height="2"/>
 </tileset>
 <layer id="1" name="Fond" width="2" height="1">
  <data encoding="csv">
1,0
  </data>
 </layer>
 <layer id="2" name="Dessus" width="2" height="1">
  <data encoding="csv">
0,2
  </data>
 </layer>
 <objectgroup id="3" name="Entités">
  <object id="1" name="Joueur" class="player" x="400" y="400" width="4" height="4"/>
 </objectgroup>
</map>"#;
        let rapport = import_tmx_str(xml, "deux_couches.tmx").expect("deux couches");
        assert_eq!(rapport.tile_layers_imported, 2);
        assert_eq!(rapport.tile_layers_flattened, 0);

        let mut z: Vec<i32> = rapport
            .scene
            .entities
            .iter()
            .filter(|entite| entite.tag == "tilemap")
            .filter_map(|entite| entite.renderable.as_ref().map(|rendu| rendu.z_index))
            .collect();
        assert_eq!(z.len(), 2, "une entité Tilemap par couche");
        let fond = z.remove(0);
        let dessus = z.remove(0);
        assert!(
            dessus > fond,
            "la couche du dessous doit passer au-dessus : {fond} < {dessus}"
        );
    }

    #[test]
    fn test_q_preflight_tileset_manquant_est_bloquant() {
        let carte = carte_temporaire(
            "tilemap_preflight",
            &["tilemap_couleurs.tmx", "tilesets_petit.png"],
        );
        let rapport = import_tmx(&carte).expect("carte couleurs");
        let mut scene = rapport.scene.clone();
        for entite in &mut scene.entities {
            if let Some(rendu) = &mut entite.renderable {
                if let ShapeKind::Tilemap { asset, .. } = &mut rendu.shape {
                    *asset = "/chemin/inexistant/tileset.png".to_string();
                }
            }
        }
        let etat = GameState::from_scene_data(scene, "scene_preflight.json")
            .expect("état avec atlas manquant");
        let prevol = StandaloneExporter::preflight_with_runtime_dir(&etat, None);
        assert!(!prevol.ok);
        assert!(
            prevol
                .issues
                .iter()
                .any(|issue| issue.code == "asset-manquant" && issue.message.contains("tileset")),
            "diagnostics : {:?}",
            prevol.issues
        );

        let _ = std::fs::remove_dir_all(carte.parent().expect("dossier temporaire"));
    }

    #[test]
    fn test_s_carte_multi_lignes_csv_importee() {
        // Régression : le crate vendoré refuse les CSV sur plusieurs lignes, et
        // joindre les rangées sans virgule collait « 4 » et « 1 » en « 41 ».
        // Une carte réelle a une ligne par rangée : elle doit s'importer.
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<map version="1.10" orientation="orthogonal" width="4" height="3" tilewidth="16" tileheight="16" infinite="0">
 <tileset firstgid="1" name="t" tilewidth="16" tileheight="16" tilecount="4" columns="4">
  <image source="atlas.png" width="64" height="16"/>
 </tileset>
 <layer id="1" name="Sol" width="4" height="3">
  <data encoding="csv">
1,2,3,4
4,3,2,1
1,1,1,1
  </data>
 </layer>
 <objectgroup id="2" name="Objets">
  <object id="1" class="player" x="0" y="0" width="16" height="16"/>
 </objectgroup>
</map>"#;
        let rapport = import_tmx_str(xml, "multi_lignes.tmx").expect("CSV multi-lignes accepté");
        let tilemap = rapport
            .scene
            .entities
            .iter()
            .find(|entite| matches!(
                entite.renderable.as_ref().map(|rendu| &rendu.shape),
                Some(crate::entity::ShapeKind::Tilemap { .. })
            ))
            .expect("une entité Tilemap");
        let crate::entity::ShapeKind::Tilemap { tiles, .. } =
            &tilemap.renderable.as_ref().unwrap().shape
        else {
            unreachable!()
        };
        assert_eq!(tiles.len(), 12, "4 × 3 tuiles");
        assert_eq!(&tiles[..4], &[1, 2, 3, 4], "première rangée dans l'ordre");
        assert_eq!(&tiles[4..8], &[4, 3, 2, 1], "deuxième rangée dans l'ordre");
    }

    #[test]
    fn test_t_carte_csv_incoherente_erreur_sans_panic() {
        // Le crate vendoré paniquait (« index out of bounds ») sur une carte dont le
        // CSV ne contient pas largeur × hauteur valeurs. Une entrée utilisateur ne
        // doit jamais faire tomber le processus.
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<map version="1.10" orientation="orthogonal" width="4" height="3" tilewidth="16" tileheight="16" infinite="0">
 <tileset firstgid="1" name="t" tilewidth="16" tileheight="16" tilecount="4" columns="4">
  <image source="atlas.png" width="64" height="16"/>
 </tileset>
 <layer id="1" name="Sol" width="4" height="3">
  <data encoding="csv">1,2,3</data>
 </layer>
 <objectgroup id="2" name="Objets">
  <object id="1" class="player" x="0" y="0" width="16" height="16"/>
 </objectgroup>
</map>"#;
        let resultat = import_tmx_str(xml, "incoherente.tmx");
        assert!(resultat.is_err(), "une carte incohérente doit renvoyer une erreur");
    }
}
