"""Render the site's hero screenshot (public/app-screenshot.webp) from a Corvene
build with the `snapshots` feature, over its control socket (tools/parity).

    cargo build --release -p corvene --features snapshots
    python3 site/scripts/screenshot.py <scratch dir> [dark|light]

Writes <scratch dir>/shot-<theme>.png (1367x814 at 2x, the empty title bar
cropped). Encode it with `cwebp -q 90 -m 6 -sharp_yuv … -o site/public/app-screenshot.webp`.
Run from the repository root, or set CORVENE_ROOT.
"""
import os
import shutil
import sys
import time
from pathlib import Path

ROOT = Path(os.environ.get("CORVENE_ROOT", Path.cwd())).resolve()
assert (ROOT / "Cargo.toml").exists(), "run from the repository root or set CORVENE_ROOT"
sys.path.insert(0, str(ROOT / "tools" / "parity"))
import drivers  # noqa: E402
import fixture  # noqa: E402

scratch = Path(sys.argv[1])
theme = sys.argv[2] if len(sys.argv) > 2 else "dark"
scratch.mkdir(parents=True, exist_ok=True)

repo = fixture.build(scratch / "fx")
demo = scratch / "fx" / "hello-rust"
if demo.exists():
    fixture.remove_tree(demo)
shutil.move(str(repo), str(demo))

# a more substantial change than the fixture's, for the picture
(demo / "src" / "main.rs").write_text(
    '''mod cli;

use cli::Options;

fn main() {
    let options = Options::parse(std::env::args().skip(1));
    let name = options
        .name
        .or_else(|| std::env::var("USER").ok())
        .unwrap_or_else(|| "world".into());
    let line = format!("Hello, {name}!");
    if options.shout {
        println!("{}", line.to_uppercase());
    } else {
        println!("{line}");
    }
}
'''
)
(demo / "src" / "cli.rs").write_text(
    '''/// Command line options: `hello-rust [--shout] [NAME]`.
#[derive(Debug, Default)]
pub struct Options {
    pub shout: bool,
    pub name: Option<String>,
}

impl Options {
    pub fn parse(args: impl Iterator<Item = String>) -> Self {
        let mut options = Options::default();
        for arg in args {
            match arg.as_str() {
                "--shout" | "--loud" => options.shout = true,
                _ if !arg.starts_with("--") => options.name = Some(arg),
                other => eprintln!("unknown flag: {other}"),
            }
        }
        options
    }
}
'''
)



def crop_title_bar(path: Path) -> None:
    """The offscreen render has no traffic lights: drop the empty title bar above the toolbar."""
    from PIL import Image

    image = Image.open(path).convert("RGB")
    width, height = image.size
    toolbar = (36, 41, 46)  # GitHub Desktop's dark toolbar
    y = 0
    while y < height and any(abs(a - b) > 10 for a, b in zip(image.getpixel((8, y)), toolbar)):
        y += 1
    if 0 < y < height // 10:
        image.crop((0, y, width, height)).save(path)


os.environ["PARITY_CORVENE_FLAGS"] = "preset=corvene"
binary = ROOT / "target" / "release" / "corvene"
data = scratch / f"data-{theme}"
if data.exists():
    shutil.rmtree(data)
cv = drivers.Corvene(binary, data, scratch / f"corvene-{theme}.log", theme)
cv.start(timeout=60)
try:
    info = cv.resize(1367, 814)
    print("window", info)
    cv.hook("complete-welcome")
    cv.hook("add-repo", str(demo))
    cv.cmd("bench", steps=[], until="idle", timeout_ms=30000)
    print("state", cv.cmd("state"))
    # GHD sorts changed files case-insensitively by path
    files = sorted(["README.md", "docs/old.md", "notes.txt", "src/cli.rs", "src/lib.rs", "src/main.rs"], key=str.lower)
    index = files.index("src/main.rs")
    row_top = float(os.environ.get("ROW_TOP", "108"))
    y = row_top + 29 * index + 14
    cv.click(150, y)
    try:
        cv.cmd("bench", steps=[], until="diff:src/main.rs", timeout_ms=8000)
    except RuntimeError as err:
        print("select failed:", err, "state", cv.cmd("state"))
    time.sleep(2.5)  # syntax highlighting settles
    print("state", cv.cmd("state"))
    out = scratch / f"shot-{theme}.png"
    cv.snap(out)
    crop_title_bar(out)
    print("wrote", out)
finally:
    cv.stop()
