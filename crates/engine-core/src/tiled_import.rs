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
//! - les **couches de tuiles** sont **aplaties** en une seule entité `decor`
//!   rectangulaire : les tuiles individuelles et les tilesets ne sont pas importés ;
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
/// Couleurs de fond stables attribuées aux couches de tuiles aplaties.
const COULEURS_COUCHES: [u32; 6] = [
    0x00334155, 0x002A3A2F, 0x003A2F2A, 0x002F2A3A, 0x003A3A2A, 0x002A3A3A,
];

/// Rapport d'import d'une carte Tiled.
#[derive(Debug, Clone)]
pub struct TiledImportReport {
    /// Scène prête à être sérialisée puis chargée par `GameState`.
    pub scene: SceneData,
    /// Avertissements non bloquants (tuiles aplaties, couches ignorées, etc.).
    pub warnings: Vec<String>,
    /// Nombre total d'entités produites (objets + couches de tuiles aplaties).
    pub entities_imported: usize,
    /// Nombre de couches de tuiles transformées en entité de décor.
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
         l'import v1 n'importe pas les tilesets mais le parseur Tiled exige leur présence."
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
/// `.tmx` (comportement du crate `tiled`), même s'ils ne sont pas importés dans la
/// scène v1.
pub fn import_tmx(path: &Path) -> Result<TiledImportReport, TiledImportError> {
    let xml = std::fs::read_to_string(path).map_err(|source| TiledImportError::Lecture {
        chemin: path.display().to_string(),
        source,
    })?;
    let origin = path.display().to_string();
    import_impl(&xml, &origin, Some(path))
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
/// rester un nom de fichier portable. Aucun fichier n'est écrit ailleurs : les
/// assets du dépôt ne sont jamais modifiés.
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
pub fn import_tmx_str(xml: &str, origin: &str) -> Result<TiledImportReport, TiledImportError> {
    import_impl(xml, origin, None)
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
    let mut couches_tuiles_aplaties = 0usize;

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
            "Carte Tiled infinie : les couches de tuiles ne sont pas aplaties (les objets sont \
             importés tels quels). Utilisez une carte finie pour obtenir un décor."
                .to_string(),
        );
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

                let largeur = largeur_tuiles as f32 * map.tile_width as f32;
                let hauteur = hauteur_tuiles as f32 * map.tile_height as f32;
                let nom_couche = if couche.name.trim().is_empty() {
                    format!("couche_{}", index_couche)
                } else {
                    couche.name.clone()
                };
                let couleur = COULEURS_COUCHES[index_couche % COULEURS_COUCHES.len()];
                let mut rendu = Renderable2D::rect(couleur, -(index_couche as i32) - 1);
                rendu.outline_color = Some(0x00475569);
                rendu.outline_width = 1;

                let entite = Entity::new(
                    prochain_id,
                    format!("tiled:{}", nom_couche),
                    "decor",
                    Transform2D::new(couche.offset_x, couche.offset_y, largeur, hauteur),
                )
                .with_renderable(rendu);

                entites.push(entite);
                prochain_id += 1;
                couches_tuiles_aplaties += 1;
                warnings.push(format!(
                    "Couche de tuiles « {} » aplatie en une entité de décor — les tuiles \
                     individuelles ne sont pas importées en v1.",
                    nom_couche
                ));
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
             de tuiles aplaties)."
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
        tile_layers_flattened: couches_tuiles_aplaties,
        map_size: (
            map.width.saturating_mul(map.tile_width),
            map.height.saturating_mul(map.tile_height),
        ),
    })
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
    use crate::game::GameState;
    use std::path::PathBuf;

    fn fixture(nom: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(nom)
    }

    #[test]
    fn test_a_carte_minimale_joueur_murs_porte() {
        let rapport = import_tmx(&fixture("minimal.tmx")).expect("carte minimale importable");
        assert!(rapport.warnings.is_empty(), "{:?}", rapport.warnings);
        assert_eq!(rapport.entities_imported, 4);
        assert_eq!(rapport.scene.entities.len(), 4);
        assert_eq!(rapport.map_size, (800, 608));
        assert_eq!(rapport.tile_layers_flattened, 0);

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
    fn test_d_couche_de_tuiles_aplatie_un_avertissement() {
        let rapport = import_tmx(&fixture("couche_tuiles.tmx")).expect("carte tuiles");
        assert_eq!(rapport.tile_layers_flattened, 1);
        assert_eq!(rapport.entities_imported, 2);
        assert_eq!(
            rapport.warnings.len(),
            1,
            "avertissements : {:?}",
            rapport.warnings
        );
        assert!(rapport.warnings[0].contains("aplatie"));
        assert!(rapport.warnings[0].contains("tuiles individuelles"));

        let decor = rapport
            .scene
            .entities
            .iter()
            .find(|entite| entite.tag == "decor")
            .expect("entité de décor");
        assert_eq!(decor.name, "tiled:Sol");
        assert_eq!(
            (
                decor.transform.x,
                decor.transform.y,
                decor.transform.w,
                decor.transform.h
            ),
            (0.0, 0.0, 128.0, 96.0)
        );
        assert_eq!(
            decor.renderable.as_ref().expect("rendu").shape,
            ShapeKind::Rect
        );
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
        assert_eq!(importe.route.tile_layers_flattened, 1);

        let projet = KaractiProject::load_from_file(&importe.project_path).expect("projet relu");
        assert_eq!(projet.scene.entities.len(), importe.route.entities_imported);
        assert_eq!(projet.metadata.title, "carte_dest");

        let _ = std::fs::remove_dir_all(&dossier);
    }
}
