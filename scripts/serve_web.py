#!/usr/bin/env python3
import http.server
import socketserver
import os
import sys

PORT = 8080
WEB_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "web"))

class CustomHandler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=WEB_DIR, **kwargs)

    def end_headers(self):
        # Permettre SharedArrayBuffer et l'isolation cross-origin si nécessaire
        self.send_header("Cross-Origin-Opener-Policy", "same-origin")
        self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
        self.send_header("Cache-Control", "no-cache, must-revalidate")
        super().end_headers()

    def guess_type(self, path):
        if path.endswith(".wasm"):
            return "application/wasm"
        if path.endswith(".js") or path.endswith(".mjs"):
            return "application/javascript"
        return super().guess_type(path)

def run():
    socketserver.TCPServer.allow_reuse_address = True
    with socketserver.TCPServer(("", PORT), CustomHandler) as httpd:
        print("=" * 60)
        print(f"🚀 SERVEUR DE DÉMONSTRATION WEB KARACTIRAGE ACTIF")
        print(f"📍 URL locale : http://localhost:{PORT}")
        print(f"📁 Dossier servi : {WEB_DIR}")
        print("💡 Appuyez sur Ctrl+C pour arrêter le serveur.")
        print("=" * 60)
        try:
            httpd.serve_forever()
        except KeyboardInterrupt:
            print("\nArrêt du serveur.")

if __name__ == "__main__":
    run()
