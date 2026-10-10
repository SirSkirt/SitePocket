#!/usr/bin/env python3
"""Register WebCask Activity + native logger after Tauri Android initialization."""
from pathlib import Path
import re
import xml.etree.ElementTree as ET

source = Path("src-tauri/android")
app = Path("src-tauri/gen/android/app")
main = app / "src/main"
activities = list((main / "java").rglob("MainActivity.kt"))
if len(activities) != 1:
    raise SystemExit(f"Expected one MainActivity.kt, found {len(activities)}")
activity = activities[0]
match = re.search(r"(?m)^\s*package\s+([\w.]+)\s*$", activity.read_text())
if not match:
    raise SystemExit("Could not read generated Android Kotlin package")
package = match.group(1)
print("Android WebCask package:", package)

for filename in ("WebCaskActivity.kt", "WebCaskNativeLog.kt", "WebCaskSplitInitializer.kt"):
    path = source / filename
    if not path.is_file():
        raise SystemExit(f"Missing native Android source: {path}")
    (activity.parent / filename).write_text(
        path.read_text().replace("__PACKAGE__", package)
    )
    print("Installed", filename)

split = main / "res/xml/webcask_split_config.xml"
split.parent.mkdir(parents=True, exist_ok=True)
split.write_bytes((source / "webcask_split_config.xml").read_bytes())

android_ns = "http://schemas.android.com/apk/res/android"
tools_ns = "http://schemas.android.com/tools"
ET.register_namespace("android", android_ns)
ET.register_namespace("tools", tools_ns)
manifest = main / "AndroidManifest.xml"
tree = ET.parse(manifest)
application = tree.getroot().find("application")
if application is None:
    raise SystemExit("AndroidManifest is missing its application element")
android_name = f"{{{android_ns}}}name"
for element in application.findall("activity"):
    if element.get(android_name) == f"{package}.WebCaskActivity":
        raise SystemExit("WebCaskActivity already registered")
# Game WebView must SURVIVE rotation/resizing. The generated primary Tauri
# Activity has its own configChanges but that is NOT inherited by this new
# manifest Activity. If omitted Android destroys/recreates WebCaskActivity and
# Wry's old Rust-owned game WebView cannot restore into that new Activity.
# Keep the list focused on size/orientation and keyboard configuration changes.
player_configs = (
    "orientation|screenSize|smallestScreenSize|screenLayout|"
    "keyboardHidden|keyboard"
)
player = ET.SubElement(application, "activity", {
    android_name: f"{package}.WebCaskActivity",
    f"{{{android_ns}}}exported": "false",
    f"{{{android_ns}}}configChanges": player_configs,
})
if player.get(f"{{{android_ns}}}configChanges") != player_configs:
    raise SystemExit("WebCaskActivity rotation handling was not registered")
ET.SubElement(application, "property", {
    android_name: "android.window.PROPERTY_ACTIVITY_EMBEDDING_SPLITS_ENABLED",
    f"{{{android_ns}}}value": "true",
})
provider = ET.SubElement(application, "provider", {
    android_name: "androidx.startup.InitializationProvider",
    f"{{{android_ns}}}authorities": package + ".androidx-startup",
    f"{{{android_ns}}}exported": "false",
    f"{{{tools_ns}}}node": "merge",
})
ET.SubElement(provider, "meta-data", {
    android_name: f"{package}.WebCaskSplitInitializer",
    f"{{{android_ns}}}value": "androidx.startup",
})
tree.write(manifest, encoding="utf-8", xml_declaration=True)

gradle = app / "build.gradle.kts"
if not gradle.is_file():
    raise SystemExit("Android Gradle build file is missing")
gradle.write_text(
    gradle.read_text()
    + '\n// WebCask mobile secondary Activity + native logger\n'
    + 'dependencies {\n'
    + '    implementation("androidx.window:window:1.5.0")\n'
    + '    implementation("androidx.startup:startup-runtime:1.2.0")\n'
    + '}\n'
)
# Fail the Android workflow if a future generator overwrite silently strips
# rotation handling and creates a black screen again.
verify = ET.parse(manifest).getroot().find("application")
player_entries = [
    el for el in verify.findall("activity")
    if el.get(android_name) == f"{package}.WebCaskActivity"
]
if len(player_entries) != 1 or player_entries[0].get(
    f"{{{android_ns}}}configChanges"
) != player_configs:
    raise SystemExit("Generated WebCaskActivity is missing orientation/size configChanges")
print("Registered WebCask player Activity, native debugger, and rotation-safe manifest.")
