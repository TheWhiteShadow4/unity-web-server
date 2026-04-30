# Unity WebGL Development Server

A lightweight development server for Unity WebGL builds with automatic build detection and responsive UI.

## 🚀 Quick Start (Windows)

1. Download the [latest release](https://github.com/TheWhiteShadow4/unity-web-server/releases)
2. Place `unity-web-server.exe` next to your Unity WebGL build folder or directly inside:
3. Start `unity-web-server.exe` - it will:
   - Automatically try to detect your Unity build
   - Open your default browser
   - Show your game with a responsive UI

That's it! Your game is now running locally.

## 🎮 User Features

- **Auto-Detection**: Finds your Unity build automatically
- **Responsive UI**: Dark mode interface with size controls
- **Fullscreen Support**: One-click fullscreen mode
- **Size Controls**: Auto-detect or manual size adjustment
- **Quick Access**: "Open in new tab" option
- **Smart Port Selection**: Automatically finds next available port if default is in use

---

## 👩‍💻 Developer Information

### Command Line Options

```bash
unity-web-server [OPTIONS]

Options:
    -p, --port <PORT>      Server port [default: 8080, auto-increments if busy]
    -H, --host <HOST>      Host address [default: 127.0.0.1]
    -n, --headless        Start without opening browser
    -h, --help            Show help
```

### Port Selection

The server will:
1. Try to use the specified port (default: 8080)
2. If the port is busy, automatically try the next port (8081, 8082, etc.)
3. Continue up to 10 ports higher than the specified port
4. Display a clear message if using a different port than requested

### Build Search Paths

The server looks for Unity builds in:
1. `../Build`
2. `.` (current directory)
3. `../build`
4. `./Build`
5. `./build`

#### Examples
```
YourGame/
├── Build/           <- Your Unity WebGL build
│   ├── index.html
│   └── ...
└── unity-web-server.exe <- The server executable
```

```
Build/
├── index.html
├── unity-web-server.exe
└── ...
```

### Technical Features

- 🔍 Automatic build & canvas size detection
- 📦 Brotli (.br) and Gzip (.gz) compression support
- 🔒 Proper MIME types and CORS settings
- 🌐 Health check endpoint (`/health`)

### Development

```bash
# Debug build and run
cargo run

# Release build
cargo build --release

# Run tests
cargo test
```

### MIME Types Support

Handles all Unity WebGL-specific file types:
- `.wasm` → `application/wasm`
- `.js` → `application/javascript`
- `.data` → `application/octet-stream`
- `.symbols.json` → `application/json`

### Compression

Automatic detection and correct serving of:
- Brotli compressed files (.br)
- Gzip compressed files (.gz)

## License

GNU General Public License v3.0