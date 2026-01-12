#!/bin/bash

# Simple HTTP server for testing video streaming
# Serves the outputs directory with CORS headers

PORT=${1:-8080}

echo "Starting HTTP server on port $PORT"
echo ""
echo "Player URL: http://localhost:$PORT/player/"
echo ""
echo "Available sources (after encoding):"
ls -1 ../outputs/ 2>/dev/null | while read dir; do
    if [ -d "../outputs/$dir" ] && [ "$(ls -A ../outputs/$dir 2>/dev/null)" ]; then
        echo "  - $dir: http://localhost:$PORT/outputs/$dir/hls/master.m3u8"
    fi
done
echo ""
echo "Press Ctrl+C to stop"

# Check if python3 is available
if command -v python3 &> /dev/null; then
    cd "$(dirname "$0")/.."
    python3 -c "
import http.server
import socketserver
import os

PORT = $PORT

import json

class CORSRequestHandler(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header('Access-Control-Allow-Origin', '*')
        self.send_header('Access-Control-Allow-Methods', 'GET, OPTIONS')
        self.send_header('Access-Control-Allow-Headers', 'Content-Type, Range')
        self.send_header('Access-Control-Expose-Headers', 'Content-Length, Content-Range')
        self.send_header('Cache-Control', 'no-cache')
        super().end_headers()

    def do_OPTIONS(self):
        self.send_response(200)
        self.end_headers()

    def do_GET(self):
        # API endpoint to get encoding status
        if self.path == '/api/videos':
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.end_headers()

            videos = []
            outputs_dir = 'outputs'
            if os.path.exists(outputs_dir):
                for name in os.listdir(outputs_dir):
                    dir_path = os.path.join(outputs_dir, name)
                    if os.path.isdir(dir_path):
                        hls_manifest = os.path.join(dir_path, 'hls', 'master.m3u8')
                        dash_manifest = os.path.join(dir_path, 'dash', 'manifest.mpd')
                        has_hls = os.path.exists(hls_manifest)
                        has_dash = os.path.exists(dash_manifest)
                        segments_dir = os.path.join(dir_path, 'segments')
                        is_encoding = os.path.exists(segments_dir) and not (has_hls or has_dash)

                        videos.append({
                            'name': name,
                            'status': 'ready' if has_hls else ('encoding' if is_encoding else 'pending'),
                            'hls': has_hls,
                            'dash': has_dash
                        })

            self.wfile.write(json.dumps({'videos': videos}).encode())
            return

        return super().do_GET()

    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        '.m3u8': 'application/vnd.apple.mpegurl',
        '.mpd': 'application/dash+xml',
        '.m4s': 'video/iso.segment',
        '.mp4': 'video/mp4',
    }

with socketserver.TCPServer(('', PORT), CORSRequestHandler) as httpd:
    print(f'Server running at http://localhost:{PORT}/')
    httpd.serve_forever()
"
else
    echo "Python3 not found. Please install Python3 or use another HTTP server."
    exit 1
fi
