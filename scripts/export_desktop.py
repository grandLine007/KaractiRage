#!/usr/bin/env python3
"""
KaractiRage — Packageur & Exportateur Standalone Desktop (macOS, Linux, Windows)
Génère une distribution native 1-clic prête à exécuter sans installation préalable.
"""

import os
import sys
import shutil
import platform
import argparse
import subprocess
import zipfile

def log(msg):
    print(f"[KaractiRage Exporter] {msg}")

def main():
    parser = argparse.ArgumentParser(description="Packageur Standalone Desktop KaractiRage")
    parser.add_argument("--package", type=str, default=None, help="Chemin vers un fichier .karacti.json personnalisé")
    parser.add_argument("--output", type=str, default="dist", help="Dossier de sortie pour la distribution")
    parser.add_argument("--zip", action="store_true", help="Compresser le bundle généré en archive .zip")
    parser.add_argument("--name", type=str, default="KaractiRage_Game", help="Nom du bundle de jeu")
    args = parser.parse_args()

    root_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    system_name = platform.system().lower()
    bundle_name = f"{args.name}_{system_name}"
    dist_dir = os.path.join(root_dir, args.output, bundle_name)

    log("=======================================================")
    log("📦 PACKAGEUR STANDALONE DESKTOP 1-CLIC — KARACTIRAGE")
    log("=======================================================")
    log(f"Plateforme cible détectée : {platform.system()} ({platform.machine()})")
    log(f"Dossier de destination : {dist_dir}")

    # 1. Compilation native en mode release
    log("⚙️  Compilation du binaire natif optimisé (Release)...")
    compile_cmd = ["cargo", "build", "--release", "-p", "engine-core", "--bin", "karactirage"]
    res = subprocess.run(compile_cmd, cwd=root_dir)
    if res.returncode != 0:
        log("❌ Erreur lors de la compilation cargo.")
        sys.exit(res.returncode)

    # 2. Préparation du répertoire de sortie
    if os.path.exists(dist_dir):
        shutil.rmtree(dist_dir)
    os.makedirs(dist_dir, exist_ok=True)

    # 3. Copie du binaire
    bin_name = "karactirage.exe" if system_name == "windows" else "karactirage"
    src_bin = os.path.join(root_dir, "target", "release", bin_name)
    dst_bin = os.path.join(dist_dir, bin_name)

    if not os.path.exists(src_bin):
        log(f"❌ Binaire introuvable : {src_bin}")
        sys.exit(1)

    shutil.copy2(src_bin, dst_bin)
    os.chmod(dst_bin, 0o755)
    log(f"✅ Binaire copié : {bin_name}")

    # 4. Copie des assets essentiels
    src_assets = os.path.join(root_dir, "assets")
    dst_assets = os.path.join(dist_dir, "assets")
    if os.path.exists(src_assets):
        shutil.copytree(src_assets, dst_assets)
        log("✅ Répertoire assets/ inclus.")

    # 5. Inclusion du package custom si spécifié
    if args.package and os.path.exists(args.package):
        pkg_dst = os.path.join(dist_dir, "game.karacti.json")
        shutil.copy2(args.package, pkg_dst)
        log(f"✅ Package universel embarqué : {args.package} -> game.karacti.json")

    # 6. Génération des lanceurs de démarrage autonomes
    # Script Unix / macOS
    sh_launcher = os.path.join(dist_dir, "lancer_le_jeu.sh")
    with open(sh_launcher, "w", encoding="utf-8") as f:
        f.write("#!/bin/bash\n")
        f.write("cd \"$(dirname \"$0\")\"\n")
        f.write("./karactirage \"$@\"\n")
    os.chmod(sh_launcher, 0o755)

    # Script Windows .bat
    bat_launcher = os.path.join(dist_dir, "lancer_le_jeu.bat")
    with open(bat_launcher, "w", encoding="utf-8") as f:
        f.write("@echo off\n")
        f.write("cd /d \"%~dp0\"\n")
        f.write("start karactirage.exe %*\n")

    # Fichier README de distribution
    readme_path = os.path.join(dist_dir, "LISEZ-MOI.txt")
    with open(readme_path, "w", encoding="utf-8") as f:
        f.write("=================================================================\n")
        f.write(f"🎮 KARACTIRAGE — BUNDLE STANDALONE DESKTOP : {args.name}\n")
        f.write("=================================================================\n\n")
        f.write("Ce jeu a été conçu et exporté depuis le Studio KaractiRage.\n")
        f.write("Moteur 2D déterministe en Rust, physique SAT/AABB et logique KVL.\n\n")
        f.write("INSTRUCTIONS DE LANCEMENT :\n")
        f.write("  • macOS / Linux : Double-cliquez sur 'lancer_le_jeu.sh' ou exécutez './karactirage'\n")
        f.write("  • Windows : Double-cliquez sur 'lancer_le_jeu.bat'\n\n")
        f.write("COMMANDES EN JEU :\n")
        f.write("  • ZQSD ou Flèches : Déplacement du personnage\n")
        f.write("  • Espace / Z       : Saut (Plateforme) / Poussée\n")
        f.write("  • F                : Tir laser balistique\n")
        f.write("  • E                : Interagir (Portes, PNJ, Trésors)\n")
        f.write("  • 1 à 5 + U        : Sélection & Utilisation d'objet d'inventaire\n")
        f.write("  • TAB              : Bascule Studio Atelier / Mode Jeu\n")
        f.write("  • M                : Audio On/Off\n")
        f.write("  • N                : Radar Tactique Minimap\n")
        f.write("  • G                : Grille Spatiale O(1)\n")
        f.write("  • F4 / L           : Cycle instantané de genre (5 genres)\n")
        f.write("  • H                : Guide Pédagogique Interactif\n")
        f.write("  • R                : Réinitialiser la scène\n\n")
        f.write("Performance : 60 FPS constants, profil Low-End First, zéro allocation per-frame.\n")

    log("✅ Lanceurs et documentation générés.")

    # 7. Compression Zip si demandée
    if args.zip:
        zip_path = f"{dist_dir}.zip"
        log(f"🗜️  Création de l'archive ZIP distribuable : {zip_path}...")
        with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED) as zf:
            for root, _, files in os.walk(dist_dir):
                for file in files:
                    full_p = os.path.join(root, file)
                    rel_p = os.path.relpath(full_p, os.path.dirname(dist_dir))
                    zf.write(full_p, rel_p)
        log(f"🎉 Archive ZIP prête pour distribution immédiate : {zip_path}")

    log("=======================================================")
    log(f"✨ EXPORTATION DESKTOP RÉUSSIE AVEC SUCCÈS DANS : {dist_dir}")
    log("=======================================================")

if __name__ == "__main__":
    main()
