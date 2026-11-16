//! Décodage de fichiers **WAV** sans aucune dépendance externe.
//!
//! Le moteur audio historique est entièrement procédural (`crate::audio`) : un
//! créateur ne pouvait pas fournir ses propres sons. Kira a été écarté (arbre de
//! ~40 crates non vendorables, crates.io bloqué). Ce module comble le manque avec
//! le plus petit format libre et universel : **RIFF/WAVE PCM**, lu à la main.
//!
//! ## Formats acceptés
//!
//! | Format (`fmt`) | Bits | Interprétation |
//! |----------------|------|----------------|
//! | PCM entier (1) | 8 | non signé, centré sur 128 |
//! | PCM entier (1) | 16 | signé petit-boutiste |
//! | PCM entier (1) | 24 | signé petit-boutiste, extension de signe |
//! | PCM entier (1) | 32 | signé petit-boutiste |
//! | IEEE float (3) | 32 | `f32` petit-boutiste |
//! | Extensible (`0xFFFE`) | idem | sous-format PCM ou float uniquement |
//!
//! **Canaux** : mono (1) ou stéréo (2). Le signal est toujours rendu **mono**
//! (moyenne arithmétique des canaux), car l'appareil de sortie du moteur est
//! mono sur toutes les cibles (`tinyaudio`, `channels_count: 1`). Une moyenne
//! conserve le niveau perçu d'un contenu stéréo centré.
//!
//! **Refus explicites** : ADPCM, µ-law/A-law, MP3, Ogg/Vorbis, 3+ canaux, blocs
//! `fmt ` absents, `data` tronqué. Chaque refus est une [`WavError`] porteuse
//! d'un message français destiné au créateur — jamais de panique, jamais de
//! lecture hors bornes.
//!
//! ## Mémoire
//!
//! Un WAV est décodé **une fois** au chargement du projet puis partagé par
//! `Arc<SoundClip>` entre toutes les voix. Le coût est de 4 octets par
//! échantillon mono : une musique de 3 minutes à 44,1 kHz pèse ~31 Mio en
//! mémoire (et le double en base64 dans le HTML exporté). Les effets courts
//! (moins d'une seconde) restent négligeables.

use std::path::{Path, PathBuf};

/// Un son décodé, prêt à être mixé par [`crate::audio::AudioEngine`].
#[derive(Debug, Clone, PartialEq)]
pub struct SoundClip {
    /// Fréquence d'échantillonnage d'origine (Hz), conservée pour le
    /// rééchantillonnage vers celle de l'appareil.
    pub sample_rate: u32,
    /// Échantillons **mono** normalisés dans `[-1, 1]`.
    pub samples: Vec<f32>,
    /// Identifiant lisible (nom de fichier ou identifiant du manifeste), utile
    /// aux diagnostics et à l'export.
    pub name: String,
}

impl SoundClip {
    /// Durée du clip en secondes.
    pub fn duration_secs(&self) -> f32 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.samples.len() as f32 / self.sample_rate as f32
        }
    }

    /// `true` quand le clip ne contient aucun échantillon (fichier `data` vide).
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

/// Erreur de décodage WAV, toujours accompagnée d'un message français lisible
/// par le créateur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WavError {
    /// Le fichier n'est pas un RIFF/WAVE, ou un bloc obligatoire manque.
    EnTeteInvalide(String),
    /// Le format est lisible mais non pris en charge (codec, canaux, bits).
    FormatNonSupporte(String),
    /// Le fichier s'arrête au milieu d'un bloc ou d'une trame.
    DonneesTronquees(String),
}

impl std::fmt::Display for WavError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EnTeteInvalide(message)
            | Self::FormatNonSupporte(message)
            | Self::DonneesTronquees(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for WavError {}

/// Lit un entier 16 bits petit-boutiste, `None` s'il manque des octets.
fn lire_u16(octets: &[u8], offset: usize) -> Option<u16> {
    let fin = offset.checked_add(2)?;
    let tranche = octets.get(offset..fin)?;
    Some(u16::from_le_bytes([tranche[0], tranche[1]]))
}

/// Lit un entier 32 bits petit-boutiste, `None` s'il manque des octets.
fn lire_u32(octets: &[u8], offset: usize) -> Option<u32> {
    let fin = offset.checked_add(4)?;
    let tranche = octets.get(offset..fin)?;
    Some(u32::from_le_bytes([tranche[0], tranche[1], tranche[2], tranche[3]]))
}

/// Nom lisible d'un codec WAV, pour les messages de refus.
fn nom_codec(tag: u16) -> &'static str {
    match tag {
        2 => "ADPCM Microsoft",
        3 => "IEEE float",
        6 => "A-law",
        7 => "µ-law",
        0x11 => "IMA ADPCM",
        0x31 => "GSM 6.10",
        0x55 => "MP3",
        0xFFFE => "WAVE_FORMAT_EXTENSIBLE",
        _ => "codec inconnu",
    }
}

/// Décrit le bloc `fmt ` une fois normalisé (sous-format extensible résolu).
struct FormatWave {
    /// 1 = PCM entier, 3 = IEEE float.
    tag: u16,
    channels: u16,
    sample_rate: u32,
    bits: u16,
}

impl FormatWave {
    /// Octets par échantillon d'**un** canal.
    fn octets_par_echantillon(&self) -> usize {
        self.bits as usize / 8
    }

    /// Octets d'une trame (tous canaux confondus).
    fn octets_par_trame(&self) -> usize {
        self.channels as usize * self.octets_par_echantillon()
    }
}

/// Analyse le bloc `fmt ` (déjà borné à sa taille déclarée).
fn parser_fmt(bloc: &[u8]) -> Result<FormatWave, WavError> {
    if bloc.len() < 16 {
        return Err(WavError::EnTeteInvalide(format!(
            "bloc « fmt » trop court ({} octets, 16 minimum) : le fichier WAV est corrompu",
            bloc.len()
        )));
    }
    let tag_brut = lire_u16(bloc, 0).unwrap_or(0);
    let channels = lire_u16(bloc, 2).unwrap_or(0);
    let sample_rate = lire_u32(bloc, 4).unwrap_or(0);
    let block_align = lire_u16(bloc, 12).unwrap_or(0);
    let bits = lire_u16(bloc, 14).unwrap_or(0);

    // WAVE_FORMAT_EXTENSIBLE : le vrai codec est le début du GUID de sous-format
    // (octets 24..26), après cbSize (16), validBits (18) et channelMask (20).
    let tag = if tag_brut == 0xFFFE {
        if bloc.len() < 40 {
            return Err(WavError::EnTeteInvalide(format!(
                "bloc « fmt » extensible trop court ({} octets, 40 minimum) : sous-format illisible",
                bloc.len()
            )));
        }
        lire_u16(bloc, 24).unwrap_or(0xFFFF)
    } else {
        tag_brut
    };

    if channels == 0 {
        return Err(WavError::EnTeteInvalide(
            "le fichier déclare 0 canal audio : en-tête invalide".to_string(),
        ));
    }
    if channels > 2 {
        return Err(WavError::FormatNonSupporte(format!(
            "{channels} canaux : seuls le mono (1) et la stéréo (2) sont acceptés (le mixage est mono)"
        )));
    }
    if sample_rate == 0 || sample_rate > 384_000 {
        return Err(WavError::FormatNonSupporte(format!(
            "fréquence d'échantillonnage invalide ({sample_rate} Hz ; attendu entre 1 et 384 000 Hz)"
        )));
    }

    let supporte = matches!((tag, bits), (1, 8) | (1, 16) | (1, 24) | (1, 32) | (3, 32));
    if !supporte {
        let libelle = nom_codec(tag);
        return Err(WavError::FormatNonSupporte(format!(
            "format « {libelle} » ({bits} bits) non pris en charge : seuls PCM entier 8/16/24/32 bits et float 32 bits sont acceptés. Convertissez le son en WAV PCM dans un éditeur audio."
        )));
    }

    let format = FormatWave {
        tag,
        channels,
        sample_rate,
        bits,
    };
    let attendu = format.octets_par_trame() as u16;
    if block_align != 0 && block_align < attendu {
        return Err(WavError::EnTeteInvalide(format!(
            "bloc « fmt » incohérent : block_align {block_align} < {attendu} octets par trame"
        )));
    }
    Ok(format)
}

/// Décode un WAV **en mémoire** en [`SoundClip`] mono.
///
/// `name` est conservé tel quel dans [`SoundClip::name`] (nom de fichier ou
/// identifiant du manifeste) : il ne sert qu'aux messages et à l'export.
///
/// Aucune panique : toute anomalie renvoie une [`WavError`].
pub fn decode_wav(bytes: &[u8], name: &str) -> Result<SoundClip, WavError> {
    if bytes.len() < 12 {
        return Err(WavError::EnTeteInvalide(format!(
            "fichier trop court ({} octets) : un WAV commence par « RIFF????WAVE »",
            bytes.len()
        )));
    }
    if &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(WavError::EnTeteInvalide(
            "en-tête invalide : signature « RIFF…WAVE » attendue (le fichier n'est pas un WAV)".to_string(),
        ));
    }

    let mut format: Option<FormatWave> = None;
    let mut donnees: Option<&[u8]> = None;
    let mut position = 12usize;

    while position < bytes.len() {
        // Un bloc = identifiant (4) + taille (4). Une fin de fichier nette est
        // tolérée ; un identifiant à moitié écrit ne l'est pas.
        if bytes.len() - position < 8 {
            if bytes.len() == position {
                break;
            }
            return Err(WavError::DonneesTronquees(format!(
                "bloc incomplet à l'octet {position} : en-tête de bloc tronqué"
            )));
        }
        let identifiant = &bytes[position..position + 4];
        let taille = lire_u32(bytes, position + 4).unwrap_or(0) as usize;
        let debut = position + 8;
        let fin = debut.checked_add(taille).ok_or_else(|| {
            WavError::DonneesTronquees("taille de bloc démesurée : en-tête corrompu".to_string())
        })?;

        if identifiant == b"fmt " {
            if fin > bytes.len() {
                return Err(WavError::DonneesTronquees(format!(
                    "bloc « fmt » tronqué : {taille} octets annoncés, {} disponible(s)",
                    bytes.len() - debut
                )));
            }
            format = Some(parser_fmt(&bytes[debut..fin])?);
        } else if identifiant == b"data" {
            if fin > bytes.len() {
                return Err(WavError::DonneesTronquees(format!(
                    "bloc « data » tronqué : {taille} octets annoncés, {} disponible(s) — le WAV a été coupé pendant le téléchargement ou la copie",
                    bytes.len() - debut
                )));
            }
            donnees = Some(&bytes[debut..fin]);
        }

        // Les blocs RIFF sont alignés sur 2 octets : un bloc de taille impaire
        // est suivi d'un octet de remplissage.
        position = match fin.checked_add(taille & 1) {
            Some(suivant) => suivant,
            None => break,
        };
    }

    let format = format.ok_or_else(|| {
        WavError::EnTeteInvalide(
            "bloc « fmt » absent : le fichier n'est pas un WAV PCM lisible".to_string(),
        )
    })?;
    let donnees = donnees.ok_or_else(|| {
        WavError::EnTeteInvalide(
            "bloc « data » absent : aucun échantillon audio dans le fichier".to_string(),
        )
    })?;

    let trame = format.octets_par_trame();
    if trame == 0 {
        return Err(WavError::EnTeteInvalide(
            "taille de trame nulle : en-tête « fmt » incohérent".to_string(),
        ));
    }
    if donnees.len() % trame != 0 {
        return Err(WavError::DonneesTronquees(format!(
            "bloc « data » de {} octets n'est pas un multiple de {trame} (taille d'une trame) : le dernier échantillon est incomplet",
            donnees.len()
        )));
    }

    let nb_trames = donnees.len() / trame;
    let canaux = format.channels as usize;
    let mut samples = Vec::with_capacity(nb_trames);
    for t in 0..nb_trames {
        let base = t * trame;
        let mut somme = 0.0f32;
        for c in 0..canaux {
            let offset = base + c * format.octets_par_echantillon();
            somme += decoder_echantillon(&donnees[offset..], &format);
        }
        // Moyenne arithmétique : un contenu stéréo centré garde son niveau.
        let mono = (somme / canaux as f32).clamp(-1.0, 1.0);
        samples.push(mono);
    }

    Ok(SoundClip {
        sample_rate: format.sample_rate,
        samples,
        name: name.to_string(),
    })
}

/// Décode un échantillon et le ramène dans `[-1, 1]`.
///
/// Le format a déjà été validé par [`parser_fmt`] : la tranche contient toujours
/// assez d'octets, `unwrap_or(0.0)` ne sert qu'à éviter toute panique défensive.
fn decoder_echantillon(octets: &[u8], format: &FormatWave) -> f32 {
    match (format.tag, format.bits) {
        (1, 8) => {
            let valeur = *octets.first().unwrap_or(&128) as f32;
            (valeur - 128.0) / 128.0
        }
        (1, 16) => {
            let brut = lire_u16(octets, 0).unwrap_or(0) as i16;
            brut as f32 / 32_768.0
        }
        (1, 24) => {
            let Some(trois) = octets.get(0..3) else {
                return 0.0;
            };
            // Extension de signe 24 → 32 bits, puis normalisation.
            let valeur = i32::from_le_bytes([
                0,
                trois[0],
                trois[1],
                trois[2],
            ]) >> 8;
            valeur as f32 / 8_388_608.0
        }
        (1, 32) => {
            let brut = lire_u32(octets, 0).unwrap_or(0) as i32;
            brut as f32 / 2_147_483_648.0
        }
        (3, 32) => {
            let brut = lire_u32(octets, 0).unwrap_or(0);
            let valeur = f32::from_bits(brut);
            // NaN/infini d'un fichier forgé : silence plutôt que poison dans le mix.
            if valeur.is_finite() {
                valeur.clamp(-1.0, 1.0)
            } else {
                0.0
            }
        }
        _ => 0.0,
    }
}

/// Encode des échantillons mono `[-1, 1]` en WAV PCM16 mono.
///
/// Utilisé par l'export de paquet (`audio_assets`, WAV base64) : le paquet ne
/// transporte que du PCM16, quel que soit le format d'origine (float, 24 ou 32
/// bits sont donc quantifiés). La fréquence d'échantillonnage est conservée.
pub fn encode_wav_pcm16_mono(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let sample_rate = sample_rate.max(1);
    let nb_octets = samples.len() * 2;
    let mut sortie = Vec::with_capacity(44 + nb_octets);
    sortie.extend_from_slice(b"RIFF");
    sortie.extend_from_slice(&((36 + nb_octets) as u32).to_le_bytes());
    sortie.extend_from_slice(b"WAVE");
    sortie.extend_from_slice(b"fmt ");
    sortie.extend_from_slice(&16u32.to_le_bytes());
    sortie.extend_from_slice(&1u16.to_le_bytes()); // PCM entier
    sortie.extend_from_slice(&1u16.to_le_bytes()); // mono
    sortie.extend_from_slice(&sample_rate.to_le_bytes());
    sortie.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // octets/seconde
    sortie.extend_from_slice(&2u16.to_le_bytes()); // block_align
    sortie.extend_from_slice(&16u16.to_le_bytes()); // bits
    sortie.extend_from_slice(b"data");
    sortie.extend_from_slice(&(nb_octets as u32).to_le_bytes());
    for &echantillon in samples {
        let valeur = if echantillon.is_finite() {
            echantillon.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        let quantifie = (valeur * 32_767.0).round() as i16;
        sortie.extend_from_slice(&quantifie.to_le_bytes());
    }
    sortie
}

/// Ré-encode un [`SoundClip`] en WAV PCM16 mono (format de transport du paquet).
pub fn encode_wav(clip: &SoundClip) -> Vec<u8> {
    encode_wav_pcm16_mono(&clip.samples, clip.sample_rate)
}

/// Racines où chercher les assets audio : répertoire courant (Studio), puis
/// `../../` (tests lancés depuis `crates/engine-core`), puis la racine du dépôt
/// (indépendant du cwd).
fn prefixes_racine() -> [String; 3] {
    [
        String::new(),
        "../../".to_string(),
        format!("{}/../../", env!("CARGO_MANIFEST_DIR")),
    ]
}

/// Lit les octets d'un WAV référencé par un identifiant ou un chemin.
///
/// Résolution, dans l'ordre :
/// 1. si `id` finit par `.wav`, le chemin tel quel ;
/// 2. sinon `assets/audio/{id}.wav` ;
/// 3. enfin `assets/audio/{id}` si `id` porte déjà une extension absente.
///
/// Chaque candidat est testé depuis le cwd, depuis `../../` puis depuis la
/// racine du dépôt. Sous wasm, `std::fs` échoue : la fonction renvoie `None` et
/// seuls les WAV **embarqués** du paquet sont alors disponibles (comportement
/// attendu, aucun accès disque dans le navigateur).
pub fn read_wav_source(id: &str) -> Option<Vec<u8>> {
    let id = id.trim();
    if id.is_empty() {
        return None;
    }
    let mut candidats: Vec<String> = Vec::new();
    if id.contains('/') || id.contains('\\') || id.to_ascii_lowercase().ends_with(".wav") {
        // Chemin explicite (manifeste, référence KVL) : on tente le fichier
        // tel quel, même si son extension n'est pas `.wav`, pour que le refus
        // vienne du décodeur (« en-tête invalide ») et non d'un « introuvable »
        // trompeur.
        candidats.push(id.to_string());
    }
    if !id.to_ascii_lowercase().ends_with(".wav") {
        candidats.push(format!("assets/audio/{id}.wav"));
        candidats.push(format!("assets/audio/{id}"));
    }
    for candidat in candidats {
        for prefixe in prefixes_racine() {
            if let Ok(octets) = std::fs::read(format!("{prefixe}{candidat}")) {
                return Some(octets);
            }
        }
    }
    None
}

/// Liste les WAV présents dans `assets/audio/` (un seul niveau, pas de
/// récursion) sous forme `(identifiant = nom de fichier sans extension, octets)`.
///
/// Le tri par identifiant garantit un ordre déterministe (export reproductible).
/// Utilisé par le chargement de projet natif : sous wasm, la liste est vide et
/// ce sont les WAV embarqués du paquet qui alimentent la banque de sons.
pub fn lister_wav_locaux() -> Vec<(String, Vec<u8>)> {
    let mut trouves: Vec<(String, Vec<u8>)> = Vec::new();
    for prefixe in prefixes_racine() {
        let dossier = format!("{prefixe}assets/audio");
        let Ok(entrees) = std::fs::read_dir(Path::new(&dossier)) else {
            continue;
        };
        for entree in entrees.flatten() {
            let chemin = entree.path();
            if !chemin.is_file() {
                continue;
            }
            let Some(extension) = chemin.extension().and_then(|e| e.to_str()) else {
                continue;
            };
            if !extension.eq_ignore_ascii_case("wav") {
                continue;
            }
            let Some(identifiant) = chemin.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if trouves.iter().any(|(id, _)| id == identifiant) {
                continue;
            }
            if let Ok(octets) = std::fs::read(&chemin) {
                trouves.push((identifiant.to_string(), octets));
            }
        }
        // Le premier dossier trouvé (cwd) fait foi ; les suivants ne servent
        // qu'aux tests et aux binaires lancés depuis un autre répertoire.
    }
    trouves.sort_by(|a, b| a.0.cmp(&b.0));
    trouves
}

/// Chemin canonique d'un WAV local, pour les diagnostics (`None` s'il est absent).
pub fn chemin_local(id: &str) -> Option<PathBuf> {
    let id = id.trim();
    if id.is_empty() {
        return None;
    }
    let mut candidats: Vec<String> = Vec::new();
    if id.contains('/') || id.contains('\\') || id.to_ascii_lowercase().ends_with(".wav") {
        candidats.push(id.to_string());
    }
    if !id.to_ascii_lowercase().ends_with(".wav") {
        candidats.push(format!("assets/audio/{id}.wav"));
    }
    for candidat in candidats {
        for prefixe in prefixes_racine() {
            let chemin = PathBuf::from(format!("{prefixe}{candidat}"));
            if chemin.is_file() {
                return Some(chemin);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construit un WAV PCM16 avec un en-tête écrit à la main.
    ///
    /// `entrelace` est le flux brut (canaux entrelacés) : les tests stéréo y
    /// placent des canaux distincts pour vérifier la moyenne en mono.
    fn wav_pcm16(entrelace: &[i16], sample_rate: u32, channels: u16) -> Vec<u8> {
        let block_align = channels * 2;
        let octets: Vec<u8> = entrelace.iter().flat_map(|v| v.to_le_bytes()).collect();
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&((36 + octets.len()) as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&(sample_rate * block_align as u32).to_le_bytes());
        wav.extend_from_slice(&block_align.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(octets.len() as u32).to_le_bytes());
        wav.extend_from_slice(&octets);
        wav
    }

    /// WAV minimal avec un `fmt ` arbitraire (pour tester les refus).
    fn wav_avec_format(tag: u16, bits: u16, channels: u16, donnees: &[u8]) -> Vec<u8> {
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&((36 + donnees.len()) as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&tag.to_le_bytes());
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&8_000u32.to_le_bytes());
        wav.extend_from_slice(&8_000u32.to_le_bytes());
        wav.extend_from_slice(&(channels * bits / 8).to_le_bytes());
        wav.extend_from_slice(&bits.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(donnees.len() as u32).to_le_bytes());
        wav.extend_from_slice(donnees);
        wav
    }

    /// Sinus PCM16 mono 8 kHz d'une demi-seconde (4000 échantillons).
    fn sinus_pcm16_8khz() -> Vec<u8> {
        let entrelace: Vec<i16> = (0..4000)
            .map(|n| {
                let t = n as f32 / 8_000.0;
                ((t * 440.0 * std::f32::consts::TAU).sin() * 20_000.0) as i16
            })
            .collect();
        wav_pcm16(&entrelace, 8_000, 1)
    }

    #[test]
    fn decode_pcm16_mono_8khz() {
        let clip = decode_wav(&sinus_pcm16_8khz(), "bip").expect("WAV PCM16 valide");
        assert_eq!(clip.sample_rate, 8_000);
        assert_eq!(clip.name, "bip");
        assert_eq!(clip.samples.len(), 4_000, "un échantillon par trame mono");
        assert!((clip.duration_secs() - 0.5).abs() < 1e-6);
        assert!(
            clip.samples.iter().any(|s| s.abs() > 0.1),
            "le sinus doit produire du signal"
        );
        assert!(
            clip.samples.iter().all(|s| (-1.0..=1.0).contains(s)),
            "tout échantillon doit rester dans [-1, 1]"
        );
    }

    #[test]
    fn decode_stereo_est_mixe_en_mono() {
        // Canal gauche à +0,5 et canal droit à -0,5 → moyenne nulle.
        let entrelace: Vec<i16> = std::iter::repeat([16_384i16, -16_384i16])
            .take(100)
            .flatten()
            .collect();
        let wav = wav_pcm16(&entrelace, 44_100, 2);
        let clip = decode_wav(&wav, "stereo").expect("WAV stéréo valide");
        assert_eq!(clip.sample_rate, 44_100);
        assert_eq!(clip.samples.len(), 100, "une trame stéréo → un échantillon mono");
        assert!(
            clip.samples.iter().all(|s| s.abs() < 1e-4),
            "les canaux opposés doivent s'annuler par moyenne"
        );

        // Cas non symétrique : gauche 1,0 / droite 0,0 → 0,5.
        let entrelace: Vec<i16> = std::iter::repeat([32_767i16, 0i16])
            .take(50)
            .flatten()
            .collect();
        let clip = decode_wav(&wav_pcm16(&entrelace, 8_000, 2), "stereo2").expect("WAV stéréo");
        assert_eq!(clip.samples.len(), 50);
        assert!((clip.samples[0] - 0.5).abs() < 1e-3, "moyenne attendue 0,5");
    }

    #[test]
    fn decode_refuse_les_en_tetes_invalides_sans_paniquer() {
        let err = decode_wav(b"", "vide").expect_err("fichier vide refusé");
        assert!(matches!(err, WavError::EnTeteInvalide(_)));
        assert!(err.to_string().contains("trop court"), "{err}");

        let err = decode_wav(b"RIFF\x00\x00\x00\x00AVI ", "avi").expect_err("non WAVE refusé");
        assert!(matches!(err, WavError::EnTeteInvalide(_)));
        assert!(err.to_string().contains("en-tête invalide"), "{err}");

        // En-tête tronqué à 11 octets : refus propre, pas de lecture hors bornes.
        let err = decode_wav(b"RIFF\x00\x00\x00\x00WAV", "court").expect_err("tronqué refusé");
        assert!(matches!(err, WavError::EnTeteInvalide(_)));
    }

    #[test]
    fn decode_refuse_un_format_adpcm_avec_message_clair() {
        // WAVE_FORMAT_ADPCM (2) : codec non PCM, doit être refusé explicitement.
        let wav = wav_avec_format(2, 4, 1, &[0u8; 16]);
        let err = decode_wav(&wav, "adpcm").expect_err("ADPCM refusé");
        assert!(matches!(err, WavError::FormatNonSupporte(_)));
        let message = err.to_string();
        assert!(message.contains("ADPCM"), "{message}");
        assert!(message.contains("PCM"), "{message}");
    }

    #[test]
    fn decode_refuse_les_canaux_et_bits_non_supportes() {
        let err = decode_wav(&wav_avec_format(1, 16, 3, &[0u8; 6]), "trois")
            .expect_err("3 canaux refusés");
        assert!(matches!(err, WavError::FormatNonSupporte(_)));
        assert!(err.to_string().contains("canaux"), "{err}");

        let err = decode_wav(&wav_avec_format(1, 12, 1, &[0u8; 3]), "douze")
            .expect_err("12 bits refusés");
        assert!(matches!(err, WavError::FormatNonSupporte(_)));
    }

    #[test]
    fn decode_refuse_les_donnees_tronquees() {
        let mut wav = sinus_pcm16_8khz();
        wav.truncate(wav.len() - 3); // coupe au milieu de la dernière trame
        let err = decode_wav(&wav, "coupe").expect_err("données tronquées refusées");
        assert!(matches!(err, WavError::DonneesTronquees(_)), "{err:?}");
        assert!(err.to_string().contains("data"), "{err}");
    }

    #[test]
    fn decode_pcm8_24_32_et_float32() {
        // 8 bits non signé : 128 = silence, 255 ≈ +1, 0 = -1.
        let clip = decode_wav(&wav_avec_format(1, 8, 1, &[128, 255, 0]), "u8").expect("PCM8");
        assert_eq!(clip.samples.len(), 3);
        assert!(clip.samples[0].abs() < 1e-6);
        assert!((clip.samples[1] - 0.992_187_5).abs() < 1e-6);
        assert!((clip.samples[2] + 1.0).abs() < 1e-6);

        // 24 bits signé : -8388608 (min) → -1 ; 4194304 → 0,5.
        let mut donnees24 = Vec::new();
        for valeur in [-8_388_608i32, 4_194_304, 0] {
            let octets = valeur.to_le_bytes();
            donnees24.extend_from_slice(&octets[0..3]);
        }
        let clip = decode_wav(&wav_avec_format(1, 24, 1, &donnees24), "s24").expect("PCM24");
        assert_eq!(clip.samples.len(), 3);
        assert!((clip.samples[0] + 1.0).abs() < 1e-6, "{:?}", clip.samples);
        assert!((clip.samples[1] - 0.5).abs() < 1e-6);
        assert!(clip.samples[2].abs() < 1e-6);

        // 32 bits signé : i32::MIN → -1.
        let donnees32: Vec<u8> = i32::MIN.to_le_bytes().to_vec();
        let clip = decode_wav(&wav_avec_format(1, 32, 1, &donnees32), "s32").expect("PCM32");
        assert!((clip.samples[0] + 1.0).abs() < 1e-6);

        // Float32 : 0,25 et une valeur aberrante (2,0) bornée à 1,0.
        let mut donnees_f = Vec::new();
        donnees_f.extend_from_slice(&0.25f32.to_le_bytes());
        donnees_f.extend_from_slice(&2.5f32.to_le_bytes());
        let clip = decode_wav(&wav_avec_format(3, 32, 1, &donnees_f), "f32").expect("Float32");
        assert!((clip.samples[0] - 0.25).abs() < 1e-6);
        assert!((clip.samples[1] - 1.0).abs() < 1e-6, "valeur bornée à 1,0");
    }

    #[test]
    fn decode_extensible_resout_le_sous_format_pcm() {
        // fmt extensible de 40 octets : tag 3 (float), 1 canal, 32 bits.
        let mut fmt = Vec::new();
        fmt.extend_from_slice(&0xFFFEu16.to_le_bytes());
        fmt.extend_from_slice(&1u16.to_le_bytes());
        fmt.extend_from_slice(&8_000u32.to_le_bytes());
        fmt.extend_from_slice(&32_000u32.to_le_bytes());
        fmt.extend_from_slice(&4u16.to_le_bytes());
        fmt.extend_from_slice(&32u16.to_le_bytes());
        fmt.extend_from_slice(&22u16.to_le_bytes()); // cbSize
        fmt.extend_from_slice(&32u16.to_le_bytes()); // valid bits
        fmt.extend_from_slice(&0u32.to_le_bytes()); // channel mask
        fmt.extend_from_slice(&3u16.to_le_bytes()); // sous-format IEEE float
        fmt.extend_from_slice(&[0u8; 14]);

        let donnees = 0.5f32.to_le_bytes();
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&((12 + 8 + fmt.len() + 8 + donnees.len()) as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
        wav.extend_from_slice(&fmt);
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(donnees.len() as u32).to_le_bytes());
        wav.extend_from_slice(&donnees);

        let clip = decode_wav(&wav, "extensible").expect("extensible PCM/float lisible");
        assert_eq!(clip.sample_rate, 8_000);
        assert!((clip.samples[0] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn encodage_pcm16_est_relu_par_le_decodeur() {
        let source: Vec<f32> = (0..800)
            .map(|n| (n as f32 / 800.0 * std::f32::consts::TAU).sin() * 0.8)
            .collect();
        let wav = encode_wav_pcm16_mono(&source, 16_000);
        let clip = decode_wav(&wav, "encode").expect("WAV réencodé lisible");
        assert_eq!(clip.sample_rate, 16_000);
        assert_eq!(clip.samples.len(), source.len());
        for (a, b) in source.iter().zip(&clip.samples) {
            assert!((a - b).abs() < 1e-3, "quantification PCM16 trop grossière");
        }
    }

    #[test]
    fn lecture_et_listage_des_wav_locaux() {
        // Aucun WAV binaire n'est versionné pour les tests ; le listage doit
        // simplement ne jamais paniquer, et un identifiant inexistant ne doit
        // rien trouver. (Si un projet ajoute un jour `assets/audio/`, les
        // entrées listées restent des fichiers lisibles non vides.)
        for (identifiant, octets) in lister_wav_locaux() {
            assert!(!identifiant.is_empty());
            assert!(!octets.is_empty());
        }
        assert!(read_wav_source("son_qui_n_existe_pas").is_none());
        assert!(chemin_local("son_qui_n_existe_pas").is_none());
        assert!(read_wav_source("   ").is_none());
    }
}
