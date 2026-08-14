# Backdrop

A GNOME wallpaper chooser exploring several distinct browsing layouts —
skewed carousel, stacked deck, infinite ribbon, masonry grid,
glassmorphism, parallax gallery, radial fan-out, and cylindrical ring.

Rust rewrite of a Python/PyGObject prototype, built with `gtk4-rs` and
`libadwaita`, aimed at eliminating the PyGObject marshaling overhead
that capped the Python version's carousel scroll performance.

## Status

Early scaffold — pipeline verified (compiles + links against real
GTK4/libadwaita), no real UI yet.

- [x] Cargo project + gtk4-rs/libadwaita dependency resolution
- [x] Verified compilation + linking against system GTK4/libadwaita
- [ ] SkewedCard widget (Gsk::Transform skew + cover-fit texture)
- [ ] ThumbnailLoader (async decode)
- [ ] Split-Screen layout (priority — port first)
- [ ] Remaining 7 layouts

## Build

```bash
sudo dnf install rust cargo gtk4-devel libadwaita-devel   # Fedora
# or: sudo apt install rustc cargo libgtk-4-dev libadwaita-1-dev  # Debian/Ubuntu

cargo check   # fast type-check
cargo run     # build + launch
```

If `cargo build` complains about an `edition2024` feature not being
stable (unlikely with a recent toolchain), run:

```bash
cargo update -p indexmap --precise 2.5.0
```
