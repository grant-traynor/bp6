# BERT BP6 Viz

BERT BP6 Viz is a modern, high-performance desktop application for
visualizing project schedules through a Task Breakdown (WBS) and
an interactive Gantt Chart. Built with Tauri, React, and Tailwind
CSS, it provides a seamless and responsive experience for managing
complex projects.

![Main Interface Dark Mode](docs/screenshots/main-dark.png)

## Features

- **Integrated WBS & Gantt View**: See your task hierarchy and schedule side-by-side with synchronized scrolling.
- **Critical Path Analysis**: Automatically identifies and highlights the critical path in your project schedule.
- **Dependency Management**: Visualizes "blocks" and "parent-child" relationships between tasks (beads).
- **Status Tracking**: Easily monitor task progress with color-coded status indicators (Open, In Progress, Blocked, Closed).
- **Dark/Light Modes**: Switch between themes for your preferred working environment.
- **Blazing Fast Performance**: Rust backend processes 200+ beads in <20ms (10-20x faster than JavaScript).
- **Search & Filter**: Quickly find tasks by ID, title, owner, or labels with real-time filtering.
- **Smart Algorithms**: Topological sorting, critical path finding, and Gantt layout calculation all in Rust.

## Screenshots

### Light Mode
![Main Interface Light Mode](docs/screenshots/main-light.png)

## Getting Started

### Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://www.rust-lang.org/) (latest stable)
- [Tauri Prerequisites](https://tauri.app/v1/guides/getting-started/prerequisites)

### Installation

1. Clone the repository:
   ```bash
   git clone https://github.com/your-repo/bert-viz.git
   cd bert-viz
   ```

2. Install dependencies:
   ```bash
   npm install
   ```

### Development

Start the development server:
```bash
npm run tauri dev
```

### Building

Build the production application:
```bash
npm run tauri build
```

## Architecture

BERT Viz uses a **hybrid Rust + TypeScript architecture** for optimal performance:

- **Rust Backend** (Tauri): Heavy data processing (filtering, tree building, Gantt layout, critical path)
- **TypeScript Frontend** (React): UI rendering and user interactions only
- **Performance**: 10-20x faster than pure JavaScript for 200-bead projects

See [ARCHITECTURE.md](ARCHITECTURE.md) for detailed technical documentation.

## Tech Stack

- **Frontend**: React 19, TypeScript, Tailwind CSS, Lucide React
- **Backend**: Rust, Tauri v2
- **Data**: Beads issue tracker (JSONL files)
- **Build Tool**: Vite

## Performance

All compute-intensive operations run in Rust:

| Operation | Time (200 beads) | Algorithm |
|-----------|------------------|-----------|
| Filtering | ~2-5ms | O(n) text/status/time filters |
| Tree Building | ~3-5ms | O(n + e) dependency graph |
| Topological Sort | ~2-3ms | O(n + e) Kahn's algorithm |
| Critical Path | ~2-3ms | O(n + e) longest path |
| Gantt Layout | ~3-5ms | O(n + e) position calculation |
| **Total** | **~15-25ms** | **Sub-frame rendering** |

See [BENCHMARK.md](BENCHMARK.md) for benchmarking methodology and [TESTING.md](TESTING.md) for regression testing guide.

## License

This project is licensed under the Apache License, Version 2.0. See the [LICENSE.TXT](LICENSE.TXT) file for details.
