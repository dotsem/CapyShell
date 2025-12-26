#!/usr/bin/env python3
"""
Script to rename Material Design icons and regenerate icons.slint

This script:
1. Renames icon files from "battery_full_24dp_E3E3E3_FILL0_wght400_GRAD0_opsz24.svg" 
   to just "battery_full.svg"
2. Regenerates the icons.slint file with all SVG files in the icons directory
"""

import os
import re
from pathlib import Path

# Paths
SCRIPT_DIR = Path(__file__).parent
PROJECT_ROOT = SCRIPT_DIR.parent
ICONS_DIR = PROJECT_ROOT / "material-1.0" / "ui" / "icons"
ICONS_SLINT = ICONS_DIR / "icons.slint"

# Pattern to match Material Design icon naming convention
# Matches: name_24dp_COLOR_FILL0_wght400_GRAD0_opsz24.svg
ICON_PATTERN = re.compile(
    r"^(.+?)_\d+dp_[A-Fa-f0-9]{6}_FILL\d+_wght\d+_GRAD-?\d+_opsz\d+\.svg$"
)


def get_clean_name(filename: str) -> str | None:
    """
    Extract the clean icon name from a Material Design icon filename.
    
    Args:
        filename: Original filename like "battery_full_24dp_E3E3E3_FILL0_wght400_GRAD0_opsz24.svg"
    
    Returns:
        Clean name like "battery_full.svg" or None if doesn't match pattern
    """
    match = ICON_PATTERN.match(filename)
    if match:
        return f"{match.group(1)}.svg"
    return None


def rename_icons() -> list[str]:
    """
    Rename all icons matching the Material Design naming pattern.
    
    Returns:
        List of renamed files (clean names)
    """
    renamed = []
    
    for file in ICONS_DIR.iterdir():
        if not file.is_file() or file.suffix != ".svg":
            continue
            
        clean_name = get_clean_name(file.name)
        if clean_name:
            new_path = ICONS_DIR / clean_name
            
            # Handle case where target already exists
            if new_path.exists() and new_path != file:
                print(f"Warning: {clean_name} already exists, skipping {file.name}")
                continue
                
            file.rename(new_path)
            print(f"Renamed: {file.name} -> {clean_name}")
            renamed.append(clean_name)
    
    return renamed


def generate_icons_slint() -> None:
    """
    Regenerate icons.slint with all SVG files in the icons directory.
    """
    # Collect all SVG files (excluding icons.slint itself)
    svg_files = sorted([
        f.stem for f in ICONS_DIR.iterdir()
        if f.is_file() and f.suffix == ".svg"
    ])
    
    # Generate the slint file content
    lines = [
        "// Copyright © SixtyFPS GmbH <info@slint.dev>",
        "// SPDX-License-Identifier: MIT",
        "",
        "export global Icons {",
    ]
    
    for name in svg_files:
        lines.append(f'    out property <image> {name}: @image-url("{name}.svg");')
    
    lines.append("}")
    lines.append("")  # Trailing newline
    
    # Write the file
    ICONS_SLINT.write_text("\n".join(lines))
    print(f"\nGenerated {ICONS_SLINT} with {len(svg_files)} icons")


def main():
    print(f"Icons directory: {ICONS_DIR}")
    print()
    
    # Step 1: Rename icons
    print("=== Renaming icons ===")
    renamed = rename_icons()
    if not renamed:
        print("No icons needed renaming")
    
    print()
    
    # Step 2: Regenerate icons.slint
    print("=== Regenerating icons.slint ===")
    generate_icons_slint()


if __name__ == "__main__":
    main()
