# Magnolia - Tauri + Svelte App

A desktop application built with Tauri and Svelte featuring a custom title bar that adapts to the operating system.

## Features

- **Custom Title Bar**: Native-looking title bar with OS-specific styling
  - macOS: Traffic light buttons (red, yellow, green) on the left
  - Windows/Linux: Minimize, maximize, and close buttons on the right
- **Tauri**: Lightweight Rust-based backend for native performance
- **Svelte**: Reactive frontend framework for smooth UI

## Prerequisites

Before you begin, ensure you have the following installed:
- [Node.js](https://nodejs.org/) (v16 or higher)
- [Rust](https://www.rust-lang.org/tools/install)
- System dependencies for Tauri (see [Tauri Prerequisites](https://tauri.app/v1/guides/getting-started/prerequisites))

## Getting Started

1. Install dependencies:
```bash
npm install
```

2. Run the development server:
```bash
npm run tauri:dev
```

3. Build for production:
```bash
npm run tauri:build
```

## Project Structure

```
magnolia/
├── src/                  # Svelte frontend source
│   ├── lib/             # Svelte components
│   │   └── TitleBar.svelte
│   ├── App.svelte       # Main app component
│   └── main.js          # Entry point
├── src-tauri/           # Tauri backend
│   ├── src/
│   │   └── main.rs      # Rust main file
│   ├── Cargo.toml       # Rust dependencies
│   └── tauri.conf.json  # Tauri configuration
├── package.json
└── vite.config.js       # Vite configuration
```

## Custom Title Bar

The custom title bar is implemented in `src/lib/TitleBar.svelte` and automatically detects the operating system to display the appropriate window controls:

- **macOS**: Displays traffic light buttons (close, minimize, maximize) on the left side
- **Windows/Linux**: Displays standard window buttons on the right side with hover effects

The title bar is draggable and supports all standard window operations (minimize, maximize/restore, close).

## License

MIT
