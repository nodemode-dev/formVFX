"""Writes presets/text-animators.json, formvfx's text animation preset library.

Every preset is data in the text animator preset format the engine applies in one undo step
(native/fvx-core/src/engine/cmd/textanim.rs): animators with properties, keys (seconds from the
current time), selectors (range, wiggly, expression) and their settings. Run
`./formvfx presets` after changing this file.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "presets" / "text-animators.json"

UNITS = {"Characters": 0, "Words": 2, "Lines": 3}
RAMP_UP, RAMP_DOWN, TRIANGLE, ROUND, SMOOTH, SQUARE = 1, 2, 3, 4, 5, 0

presets: list[dict] = []


def ident(name: str) -> str:
    words = re.findall(r"[A-Za-z0-9]+", name)
    return words[0].lower() + "".join(w[:1].upper() + w[1:] for w in words[1:])


def add(category: str, name: str, description: str, animators: list[dict], per_char_3d: bool = False) -> None:
    p = {"id": ident(name), "name": name, "category": category, "description": description, "animators": animators}
    if per_char_3d:
        p["perChar3d"] = True
    presets.append(p)


def reveal(unit: int, dur: float, shape: int = RAMP_UP, out: bool = False, delay: float = 0.0, extra: dict | None = None, ease: bool = True) -> dict:
    if shape == SQUARE:
        sel = {"kind": "range", "set": {"advanced/basedOn": unit, "advanced/shape": SQUARE}, "keys": {"end" if out else "start": [[delay, 0], [delay + dur, 100]]}, "ease": ease}
        if extra:
            sel["set"].update(extra)
        return sel
    keys = [[delay, -100], [delay + dur, 100]]
    sel = {"kind": "range", "set": {"advanced/basedOn": unit, "advanced/shape": RAMP_DOWN if out else shape}, "keys": {"offset": keys}, "ease": ease}
    if extra:
        sel["set"].update(extra)
    return sel


def travel(unit: int, dur: float, width: float = 30, shape: int = TRIANGLE, loop: bool = False) -> dict:
    sel = {"kind": "range", "set": {"advanced/basedOn": unit, "advanced/shape": shape, "end": width}, "keys": {"offset": [[0, -width], [dur, 100]]}, "ease": False}
    if loop:
        sel["expressions"] = {"offset": f"-{width} + ((time - inPoint) % {dur}) / {dur} * (100 + {width})"}
        sel.pop("keys")
    return sel


def wiggly(rate: float, correlation: float = 50, unit: int = 0, lo: float = -100, hi: float = 100) -> dict:
    return {"kind": "wiggly", "set": {"wigglesPerSecond": rate, "correlation": correlation, "advanced/basedOn": unit, "basedOn": unit, "minAmount": lo, "maxAmount": hi}}


def anim(name: str, props: dict, selectors: list[dict], keys: dict | None = None, ease: bool = False, expressions: dict | None = None) -> dict:
    a = {"name": name, "properties": props, "selectors": selectors}
    if keys:
        a["keys"] = keys
    if ease:
        a["ease"] = True
    if expressions:
        a["expressions"] = expressions
    return a


def per_unit(category: str, base: str, description: str, props: dict, dur: float = 1.5, shape: int = RAMP_UP, extra: dict | None = None, units: tuple = ("Characters", "Words", "Lines"), out: bool = False) -> None:
    for u in units:
        dur_u = dur if u == "Characters" else dur * 0.8
        add(category, f"{base} {u}", description.replace("{unit}", u.lower()[:-1]), [anim(base, props, [reveal(UNITS[u], dur_u, shape, out=out, extra=extra)])])


# Animate In: the text is hidden, then each unit arrives in reading order.
per_unit("Animate In", "Fade In", "Each {unit} fades in, in reading order.", {"opacity": 0})
per_unit("Animate In", "Fade Up", "Each {unit} fades in while rising into place.", {"opacity": 0, "position": [0, 40, 0]})
per_unit("Animate In", "Fade Down", "Each {unit} fades in while settling down into place.", {"opacity": 0, "position": [0, -40, 0]})
per_unit("Animate In", "Slide In Left", "Each {unit} slides in from the left while fading in.", {"opacity": 0, "position": [-140, 0, 0]}, units=("Characters", "Words", "Lines"))
per_unit("Animate In", "Slide In Right", "Each {unit} slides in from the right while fading in.", {"opacity": 0, "position": [140, 0, 0]}, units=("Characters", "Words", "Lines"))
per_unit("Animate In", "Drop In", "Each {unit} drops from above.", {"opacity": 0, "position": [0, -220, 0]}, extra={"advanced/easeHigh": 80}, units=("Characters", "Words"))
per_unit("Animate In", "Rise In", "Each {unit} rises from below.", {"opacity": 0, "position": [0, 220, 0]}, extra={"advanced/easeHigh": 80}, units=("Characters", "Words"))
per_unit("Animate In", "Scale Up", "Each {unit} grows from nothing.", {"opacity": 0, "scale": [0, 0, 100]})
per_unit("Animate In", "Scale Down", "Each {unit} shrinks into place from three times its size.", {"opacity": 0, "scale": [300, 300, 100]}, units=("Characters", "Words"))
per_unit("Animate In", "Stretch In Horizontal", "Each {unit} unfolds from a vertical sliver.", {"scale": [0, 100, 100]}, units=("Characters", "Words"))
per_unit("Animate In", "Stretch In Vertical", "Each {unit} unfolds from a horizontal sliver.", {"scale": [100, 0, 100]}, units=("Characters", "Words", "Lines"))
per_unit("Animate In", "Spin In", "Each {unit} spins half a turn into place while growing.", {"opacity": 0, "rotation": -180, "scale": [40, 40, 100]}, units=("Characters", "Words"))
per_unit("Animate In", "Skew In", "Each {unit} straightens from a slant while fading in.", {"opacity": 0, "skew": 60}, units=("Characters", "Words"))
per_unit("Animate In", "Swing In", "Each {unit} swings down from its top edge.", {"opacity": 0, "anchor": [0, -40, 0], "rotation": -90}, units=("Characters", "Words"))
per_unit("Animate In", "Typewriter", "Each {unit} appears at once, hard-edged, in reading order.", {"opacity": 0}, dur=2.0, shape=SQUARE, extra={"advanced/smoothness": 0}, units=("Characters", "Words", "Lines"))
add("Animate In", "Random Fade In", "Characters fade in in random order.", [anim("Random Fade", {"opacity": 0}, [reveal(0, 1.6, extra={"advanced/randomize": True})])])
add("Animate In", "Random Drop In", "Characters drop from above in random order.", [anim("Random Drop", {"opacity": 0, "position": [0, -200, 0]}, [reveal(0, 1.6, extra={"advanced/randomize": True, "advanced/easeHigh": 80})])])
add("Animate In", "Random Scale In", "Characters pop up from nothing in random order.", [anim("Random Scale", {"opacity": 0, "scale": [0, 0, 100]}, [reveal(0, 1.4, extra={"advanced/randomize": True})])])
add("Animate In", "Raining Characters In", "Characters fall from far above in random order and land in place.", [anim("Rain", {"opacity": 0, "position": [0, -500, 0]}, [reveal(0, 2.2, extra={"advanced/randomize": True, "advanced/easeHigh": 100})])])
add("Animate In", "Bounce In Characters", "Characters scale up with an overshoot.", [
    anim("Bounce Scale", {"scale": [0, 0, 100], "opacity": 0}, [reveal(0, 1.2, ease=False)]),
    anim("Bounce Overshoot", {"scale": [130, 130, 100]}, [{"kind": "range", "set": {"advanced/basedOn": 0, "advanced/shape": TRIANGLE, "end": 25}, "keys": {"offset": [[0.12, -25], [1.4, 100]]}}]),
])
add("Animate In", "Bounce In Lines", "Lines scale up with an overshoot, one after another.", [
    anim("Bounce Scale", {"scale": [0, 0, 100], "opacity": 0}, [reveal(3, 1.0, ease=False)]),
    anim("Bounce Overshoot", {"scale": [125, 125, 100]}, [{"kind": "range", "set": {"advanced/basedOn": 3, "advanced/shape": TRIANGLE, "end": 40}, "keys": {"offset": [[0.15, -40], [1.2, 100]]}}]),
])
add("Animate In", "Pop In Words", "Words pop to full size with a quick overshoot.", [
    anim("Pop", {"scale": [0, 0, 100]}, [reveal(2, 0.9, ease=False)]),
    anim("Pop Overshoot", {"scale": [118, 118, 100]}, [{"kind": "range", "set": {"advanced/basedOn": 2, "advanced/shape": TRIANGLE, "end": 30}, "keys": {"offset": [[0.1, -30], [1.0, 100]]}}]),
])
add("Animate In", "Slam In Words", "Words slam down from huge and blurred.", [anim("Slam", {"scale": [400, 400, 100], "blur": [30, 30], "opacity": 0}, [reveal(2, 1.0, extra={"advanced/easeHigh": 100})])])
add("Animate In", "Wipe In", "The text wipes on from left to right with a soft edge.", [anim("Wipe", {"opacity": 0}, [reveal(0, 1.2, shape=RAMP_UP, extra={"advanced/smoothness": 100})])])
add("Animate In", "Center Spread", "Characters spread out from the middle while fading in.", [anim("Spread", {"opacity": 0, "tracking": -40}, [{"kind": "range"}], keys={"opacity": [[0, 0], [0.8, 100]], "tracking": [[0, -40], [1.4, 0]]}, ease=True)])

# Animate Out: the same moves leaving.
per_unit("Animate Out", "Fade Out", "Each {unit} fades out, in reading order.", {"opacity": 0}, out=True)
per_unit("Animate Out", "Fade Out Up", "Each {unit} fades out while rising.", {"opacity": 0, "position": [0, -40, 0]}, out=True)
per_unit("Animate Out", "Fade Out Down", "Each {unit} fades out while sinking.", {"opacity": 0, "position": [0, 40, 0]}, out=True)
per_unit("Animate Out", "Slide Out Left", "Each {unit} slides away to the left while fading out.", {"opacity": 0, "position": [-140, 0, 0]}, out=True, units=("Characters", "Words"))
per_unit("Animate Out", "Slide Out Right", "Each {unit} slides away to the right while fading out.", {"opacity": 0, "position": [140, 0, 0]}, out=True, units=("Characters", "Words"))
per_unit("Animate Out", "Scale Out", "Each {unit} shrinks to nothing.", {"opacity": 0, "scale": [0, 0, 100]}, out=True)
per_unit("Animate Out", "Blow Up Out", "Each {unit} grows huge and fades away.", {"opacity": 0, "scale": [400, 400, 100]}, out=True, units=("Characters", "Words"))
per_unit("Animate Out", "Spin Out", "Each {unit} spins away while shrinking.", {"opacity": 0, "rotation": 180, "scale": [40, 40, 100]}, out=True, units=("Characters", "Words"))
per_unit("Animate Out", "Blur Out", "Each {unit} blurs and fades away.", {"opacity": 0, "blur": [40, 40]}, out=True)
per_unit("Animate Out", "Typewriter Erase", "Each {unit} disappears at once, hard-edged, in reading order.", {"opacity": 0}, dur=1.6, shape=SQUARE, extra={"advanced/smoothness": 0}, out=True, units=("Characters", "Words"))
add("Animate Out", "Random Fade Out", "Characters fade out in random order.", [anim("Random Fade", {"opacity": 0}, [reveal(0, 1.6, out=True, extra={"advanced/randomize": True})])])
add("Animate Out", "Drop Out", "Characters fall away in random order.", [anim("Drop", {"opacity": 0, "position": [0, 400, 0], "rotation": 40}, [reveal(0, 1.8, out=True, extra={"advanced/randomize": True, "advanced/easeLow": 100})])])
add("Animate Out", "Explode Out", "Characters scatter outward and fade, in random order.", [
    anim("Explode", {"opacity": 0, "position": [0, -260, 0], "rotation": 200, "scale": [160, 160, 100]}, [reveal(0, 1.4, out=True, extra={"advanced/randomize": True})]),
])
add("Animate Out", "Tracking Out", "Spacing widens while the text fades out.", [anim("Tracking Out", {"tracking": 0, "opacity": 100}, [], keys={"tracking": [[0, 0], [1.6, 400]], "opacity": [[0.8, 100], [1.6, 0]]}, ease=True)])
add("Animate Out", "Collapse Out", "Characters squeeze together and vanish.", [anim("Collapse", {"tracking": 0, "scale": [100, 100, 100], "opacity": 100}, [], keys={"tracking": [[0, 0], [1.2, -60]], "scale": [[0.4, [100, 100, 100]], [1.2, [0, 100, 100]]], "opacity": [[0.8, 100], [1.2, 0]]}, ease=True)])

# Blurs.
per_unit("Blurs", "Blur In", "Each {unit} comes into focus and fades in.", {"opacity": 0, "blur": [40, 40]})
add("Blurs", "Horizontal Blur In", "Characters streak in from a horizontal blur.", [anim("Streak", {"opacity": 0, "blur": [120, 0], "position": [-60, 0, 0]}, [reveal(0, 1.4)])])
add("Blurs", "Vertical Blur In", "Characters drop in from a vertical blur.", [anim("Streak", {"opacity": 0, "blur": [0, 120], "position": [0, -60, 0]}, [reveal(0, 1.4)])])
add("Blurs", "Blur Through", "A soft blur passes through the text from left to right.", [anim("Blur Band", {"blur": [30, 30]}, [travel(0, 1.6, 35, ROUND)])])
add("Blurs", "Blur Wave Loop", "A blur band sweeps across the text again and again.", [anim("Blur Band", {"blur": [24, 24]}, [travel(0, 2.0, 30, ROUND, loop=True)])])
add("Blurs", "Soft Focus Drift", "Characters drift in and out of focus at random.", [anim("Drift", {"blur": [18, 18]}, [{"kind": "range"}, wiggly(1.2, 30, lo=0, hi=100)])])
add("Blurs", "Focus Pull", "The whole line racks from blurred to sharp.", [anim("Focus", {"blur": [0, 0]}, [], keys={"blur": [[0, [60, 60]], [1.2, [0, 0]]]}, ease=True)])
add("Blurs", "Blur Out Random", "Characters blur away in random order.", [anim("Blur Out", {"opacity": 0, "blur": [60, 60]}, [reveal(0, 1.6, out=True, extra={"advanced/randomize": True})])])

# Rotation.
add("Rotation", "Rotate Through", "A twist travels through the characters.", [anim("Twist", {"rotation": 90}, [travel(0, 1.8, 30, ROUND)])])
add("Rotation", "Wobble", "Characters wobble back and forth, continuously.", [anim("Wobble", {"rotation": 20}, [{"kind": "range"}, wiggly(2.5, 20)])])
add("Rotation", "Pendulum Words", "Words sway like pendulums from their tops.", [anim("Pendulum", {"anchor": [0, -50, 0], "rotation": 12}, [{"kind": "range", "set": {"advanced/basedOn": 2}}, wiggly(0.8, 80, unit=2)])])
add("Rotation", "Tumble In", "Characters tumble a full turn into place in random order.", [anim("Tumble", {"rotation": 360, "opacity": 0, "position": [0, -80, 0]}, [reveal(0, 1.8, extra={"advanced/randomize": True})])])
add("Rotation", "Spin Continuous", "Every character spins in place without stopping.", [anim("Spin", {"rotation": 0}, [{"kind": "range"}], expressions={"rotation": "time * 180"})])
add("Rotation", "Tilt Wave", "A rocking wave rolls along the characters.", [anim("Tilt", {"rotation": 18}, [{"kind": "expression", "expressions": {"amount": "var s = Math.sin(time * 4 - textIndex * 0.6) * 100; [s, s, s]"}}])])
add("Rotation", "Twirl Out", "Characters twirl away upward in random order.", [anim("Twirl", {"rotation": -540, "position": [0, -160, 0], "opacity": 0}, [reveal(0, 1.8, out=True, extra={"advanced/randomize": True})])])

# Scale.
add("Scale", "Pulse", "Characters swell and shrink at random, continuously.", [anim("Pulse", {"scale": [140, 140, 100]}, [{"kind": "range"}, wiggly(2.0, 40, lo=0, hi=100)])])
add("Scale", "Zoom Through", "A swell travels through the characters.", [anim("Zoom Band", {"scale": [170, 170, 100]}, [travel(0, 1.6, 30, ROUND)])])
add("Scale", "Grow Wave Loop", "A swell rolls along the text again and again.", [anim("Wave", {"scale": [150, 150, 100]}, [travel(0, 1.8, 30, ROUND, loop=True)])])
add("Scale", "Squash and Stretch In", "Characters land squashed and spring back.", [
    anim("Stretch", {"scale": [60, 160, 100], "opacity": 0, "position": [0, -120, 0]}, [reveal(0, 1.0, ease=False)]),
    anim("Squash", {"scale": [130, 70, 100]}, [{"kind": "range", "set": {"advanced/basedOn": 0, "advanced/shape": TRIANGLE, "end": 25}, "keys": {"offset": [[0.15, -25], [1.2, 100]]}}]),
])
add("Scale", "Heartbeat", "The text beats like a heart.", [anim("Beat", {"scale": [100, 100, 100]}, [], expressions={"scale": "var b = Math.pow(Math.abs(Math.sin(time * 3.4)), 12) * 18; [100 + b, 100 + b, 100]"})])
add("Scale", "Shrink Words", "Words shrink away one after another.", [anim("Shrink", {"scale": [0, 0, 100]}, [reveal(2, 1.2, out=True)])])
add("Scale", "Scale Bounce Loop", "Characters bounce their scale in a travelling wave.", [anim("Bounce", {"scale": [100, 100, 100]}, [{"kind": "expression", "expressions": {"amount": "var s = Math.abs(Math.sin(time * 3 - textIndex * 0.5)) * 100; [s, s, s]"}}], expressions=None)])

# Tracking.
add("Tracking", "Tracking In Wide", "Very wide spacing tightens while the text fades in.", [anim("Tracking In", {"tracking": 0, "opacity": 100}, [], keys={"tracking": [[0, 600], [1.8, 0]], "opacity": [[0, 0], [0.9, 100]]}, ease=True)])
add("Tracking", "Tracking In Tight", "Overlapping characters open up to normal spacing.", [anim("Tracking In", {"tracking": 0, "opacity": 100}, [], keys={"tracking": [[0, -80], [1.4, 0]], "opacity": [[0, 0], [0.6, 100]]}, ease=True)])
add("Tracking", "Breathe", "Spacing swells and relaxes, continuously.", [anim("Breathe", {"tracking": 0}, [], expressions={"tracking": "Math.sin(time * 2) * 30 + 30"})])
add("Tracking", "Expand", "Spacing slowly widens over the whole duration.", [anim("Expand", {"tracking": 0}, [], keys={"tracking": [[0, 0], [5, 120]]})])
add("Tracking", "Collapse", "Spacing tightens until characters touch.", [anim("Collapse", {"tracking": 0}, [], keys={"tracking": [[0, 0], [1.6, -50]]}, ease=True)])
add("Tracking", "Line Spacing Open", "Lines spread apart from on top of each other.", [anim("Line Spacing", {"lineSpacing": [0, 0], "opacity": 100}, [], keys={"lineSpacing": [[0, [0, -80]], [1.4, [0, 0]]], "opacity": [[0, 0], [0.6, 100]]}, ease=True)])

# Fill and Stroke.
add("Fill and Stroke", "Hue Cycle", "The fill colour cycles through the spectrum, continuously.", [anim("Hue", {"fillHue": 0}, [], expressions={"fillHue": "time * 120"})])
add("Fill and Stroke", "Hue Wave", "A rainbow travels along the characters.", [anim("Hue Wave", {"fillHue": 0}, [{"kind": "range"}], expressions={"fillHue": "time * 90"}), anim("Hue Offset", {"fillHue": 180}, [travel(0, 2.0, 50, SMOOTH, loop=True)])])
add("Fill and Stroke", "Highlight Sweep", "A bright band sweeps across the text.", [anim("Sweep", {"fillBrightness": 80}, [travel(0, 1.4, 25, ROUND)])])
add("Fill and Stroke", "Highlight Sweep Loop", "A bright band sweeps across the text again and again.", [anim("Sweep", {"fillBrightness": 80}, [travel(0, 2.2, 25, ROUND, loop=True)])])
add("Fill and Stroke", "Desaturate In", "Characters gain their colour in reading order.", [anim("Desaturate", {"fillSaturation": -100}, [reveal(0, 1.6)])])
add("Fill and Stroke", "Fill Fade In", "The fill fades in after the text appears, in reading order.", [anim("Fill", {"fillOpacity": 0}, [reveal(0, 1.6)])])
add("Fill and Stroke", "Darken In", "Characters rise out of darkness.", [anim("Darken", {"fillBrightness": -100, "opacity": 0}, [reveal(0, 1.6)])])
add("Fill and Stroke", "Stroke Grow", "An orange outline thickens in reading order.", [anim("Stroke", {"strokeWidth": 6, "strokeColor": [0.976, 0.451, 0.086, 1]}, [reveal(0, 1.4, out=True)])])
add("Fill and Stroke", "Stroke Pulse", "An orange outline pulses in width, continuously.", [anim("Stroke", {"strokeWidth": 0, "strokeColor": [0.976, 0.451, 0.086, 1]}, [], expressions={"strokeWidth": "(Math.sin(time * 4) + 1) * 3"})])
add("Fill and Stroke", "Color Flash Random", "Characters flash white at random.", [anim("Flash", {"fillBrightness": 100}, [{"kind": "range", "set": {"advanced/randomize": True}}, wiggly(3, 0, lo=-100, hi=100)])])
add("Fill and Stroke", "Fill Color Change", "The fill changes to orange in reading order.", [anim("Recolor", {"fillColor": [0.976, 0.451, 0.086, 1]}, [reveal(0, 1.6, shape=RAMP_DOWN)])])
add("Fill and Stroke", "Fill Color Wave", "An orange band passes through the characters.", [anim("Recolor Band", {"fillColor": [0.976, 0.451, 0.086, 1]}, [travel(0, 1.8, 30, ROUND)])])

# Lights and Optical.
add("Lights and Optical", "Flash In", "Characters appear white-hot and cool to their colour.", [
    anim("Appear", {"opacity": 0}, [reveal(0, 1.2, shape=SQUARE, extra={"advanced/smoothness": 0})]),
    anim("Flash", {"fillBrightness": 100}, [travel(0, 1.4, 20, RAMP_DOWN)]),
])
add("Lights and Optical", "Light Speed In", "Characters streak in from the left like light.", [anim("Light Speed", {"opacity": 0, "position": [-400, 0, 0], "blur": [160, 0], "fillBrightness": 80}, [reveal(0, 1.2, extra={"advanced/easeHigh": 100})])])
add("Lights and Optical", "Shimmer", "Characters glint at random, continuously.", [anim("Shimmer", {"fillBrightness": 60}, [{"kind": "range"}, wiggly(4, 10, lo=0, hi=100)])])
add("Lights and Optical", "Fade From White", "The text fades in from white.", [anim("White", {"fillBrightness": 100, "opacity": 0}, [], keys={"fillBrightness": [[0, 100], [1.4, 0]], "opacity": [[0, 0], [0.5, 100]]}, ease=True)])
add("Lights and Optical", "Strobe", "Characters blink on and off at random.", [anim("Strobe", {"opacity": 0}, [{"kind": "range", "set": {"advanced/randomize": True}}, wiggly(6, 0, lo=-100, hi=100)])])
add("Lights and Optical", "Glow Pass", "A soft glowing band of light passes over the text.", [anim("Glow Band", {"fillBrightness": 70, "blur": [6, 6]}, [travel(0, 1.8, 30, ROUND)])])

# Mechanical.
add("Mechanical", "Scramble In", "Letters cycle through random characters before settling, left to right.", [anim("Scramble", {"characterOffset": 14}, [{"kind": "range", "set": {"advanced/randomize": True, "advanced/smoothness": 0}, "keys": {"start": [[0, 0], [2, 100]]}}, wiggly(8, 0)])])
add("Mechanical", "Decoder", "Characters appear as random symbols, then resolve in reading order.", [
    anim("Appear", {"opacity": 0}, [reveal(0, 1.2, shape=SQUARE, extra={"advanced/smoothness": 0})]),
    anim("Decode", {"characterOffset": 20}, [{"kind": "range", "set": {"advanced/smoothness": 0}, "keys": {"start": [[0.3, 0], [2.0, 100]]}}, wiggly(10, 0)]),
])
add("Mechanical", "Slot Machine", "Characters spin through letters and stop in reading order.", [anim("Slots", {"characterOffset": 26, "position": [0, 0, 0]}, [{"kind": "range", "set": {"advanced/smoothness": 0}, "keys": {"start": [[0, 0], [2.2, 100]]}}], expressions={"characterOffset": "Math.floor(time * 30) % 26"})])
add("Mechanical", "Glitch", "Characters jump, slant and break apart at random, continuously.", [anim("Glitch", {"position": [24, 4, 0], "skew": 30}, [{"kind": "range", "set": {"advanced/randomize": True}}, wiggly(9, 0)])])
add("Mechanical", "Glitch In", "Characters glitch into place.", [
    anim("Appear", {"opacity": 0}, [reveal(0, 1.0, shape=SQUARE, extra={"advanced/randomize": True, "advanced/smoothness": 0})]),
    anim("Glitch", {"position": [30, 0, 0], "skew": 40, "fillHue": 120}, [reveal(0, 1.6, out=True), wiggly(12, 0)]),
])
add("Mechanical", "Shutter In", "Lines open like shutters, one after another.", [anim("Shutter", {"scale": [100, 0, 100]}, [reveal(3, 1.0, shape=SQUARE, extra={"advanced/smoothness": 100})])])
add("Mechanical", "Counter Up", "Digits roll upward before settling.", [anim("Count", {"characterOffset": 0}, [{"kind": "range"}], keys={"characterOffset": [[0, 9], [1.5, 0]]})])
add("Mechanical", "Robot Steps", "Characters snap into place in hard steps.", [anim("Steps", {"position": [0, -60, 0], "opacity": 0}, [reveal(0, 1.6, shape=SQUARE, extra={"advanced/smoothness": 0}, ease=False)])])

# Organic.
add("Organic", "Wave", "Characters ride a wave, continuously.", [anim("Wave", {"position": [0, -30, 0]}, [{"kind": "expression", "expressions": {"amount": "var s = Math.sin(time * 4 - textIndex * 0.5) * 100; [s, s, s]"}}])])
add("Organic", "Float", "Characters drift gently, continuously.", [anim("Float", {"position": [0, 14, 0]}, [{"kind": "range"}, wiggly(0.6, 30)])])
add("Organic", "Jitter", "Characters wobble randomly in position and rotation, continuously.", [anim("Jitter", {"position": [0, 18, 0], "rotation": 12}, [{"kind": "range"}, wiggly(3, 20)])])
add("Organic", "Ripple", "A ripple rolls along the characters, continuously.", [anim("Ripple", {"scale": [130, 130, 100], "position": [0, -10, 0]}, [travel(0, 1.6, 30, ROUND, loop=True)])])
add("Organic", "Bubble Up", "Characters bubble up from below in random order.", [anim("Bubble", {"position": [0, 180, 0], "scale": [20, 20, 100], "opacity": 0}, [reveal(0, 2.0, extra={"advanced/randomize": True, "advanced/easeHigh": 60})])])
add("Organic", "Wind Blown", "Characters blow away to the right in random order.", [anim("Wind", {"position": [400, -60, 0], "rotation": 90, "opacity": 0}, [reveal(0, 2.0, out=True, extra={"advanced/randomize": True})])])
add("Organic", "Breathing", "The text slowly swells and relaxes, continuously.", [anim("Breath", {"scale": [100, 100, 100]}, [], expressions={"scale": "var s = 100 + Math.sin(time * 1.6) * 6; [s, s, 100]"})])
add("Organic", "Dance", "Characters hop and sway, continuously.", [anim("Dance", {"position": [0, -16, 0], "rotation": 10}, [{"kind": "expression", "expressions": {"amount": "var s = Math.abs(Math.sin(time * 5 + textIndex * 1.3)) * 100; [s, s, s]"}}])])
add("Organic", "Jelly Wobble", "Characters squash and stretch like jelly, continuously.", [anim("Jelly", {"scale": [115, 88, 100]}, [{"kind": "expression", "expressions": {"amount": "var s = Math.sin(time * 6 - textIndex * 0.7) * 100; [s, s, s]"}}])])

# Multi-Line.
add("Multi-Line", "Drop In Lines", "Lines drop from above with a slight rotation, one after another.", [anim("Drop In", {"position": [0, -120, 0], "rotation": -8, "opacity": 0}, [reveal(3, 1.4, extra={"advanced/easeHigh": 80})])])
add("Multi-Line", "Lines Wipe Up", "Lines grow up from their baselines, one after another.", [anim("Wipe Up", {"scale": [100, 0, 100], "opacity": 0}, [reveal(3, 1.2)])])
add("Multi-Line", "Lines Slide Alternate", "Lines slide in from alternating sides.", [
    anim("From Left", {"position": [-300, 0, 0], "opacity": 0}, [reveal(3, 1.4)]),
    anim("Alternate", {"position": [600, 0, 0]}, [reveal(3, 1.4), {"kind": "expression", "set": {"basedOn": 3}, "expressions": {"amount": "var s = (textIndex % 2 == 0) ? selectorValue[0] : 0; [s, s, s]"}}]),
])
add("Multi-Line", "Lines Blur In Stagger", "Lines come into focus one after another.", [anim("Blur In", {"blur": [30, 30], "opacity": 0, "position": [0, 20, 0]}, [reveal(3, 1.4)])])
add("Multi-Line", "Lines Typewriter", "Lines appear one at a time, hard-edged.", [anim("Typewriter", {"opacity": 0}, [reveal(3, 1.6, shape=SQUARE, extra={"advanced/smoothness": 0})])])
add("Multi-Line", "Lines Close Up", "Lines move together from far apart.", [anim("Close Up", {"lineSpacing": [0, 0]}, [], keys={"lineSpacing": [[0, [0, 120]], [1.4, [0, 0]]]}, ease=True)])
add("Multi-Line", "Lines Scale Pop", "Lines pop up one after another.", [anim("Pop", {"scale": [0, 0, 100]}, [reveal(3, 1.0)]), anim("Overshoot", {"scale": [115, 115, 100]}, [{"kind": "range", "set": {"advanced/basedOn": 3, "advanced/shape": TRIANGLE, "end": 40}, "keys": {"offset": [[0.12, -40], [1.1, 100]]}}])])
add("Multi-Line", "Lines Fade Out Up", "Lines fade out upward, one after another.", [anim("Fade Out", {"opacity": 0, "position": [0, -60, 0]}, [reveal(3, 1.2, out=True)])])

# Miscellaneous.
add("Miscellaneous", "Jumble", "Characters sit at random offsets and slowly settle.", [anim("Jumble", {"position": [0, 60, 0], "rotation": 30}, [{"kind": "range", "keys": {"end": [[0, 100], [2.5, 0]]}}, wiggly(0.01, 0)])])
add("Miscellaneous", "Lean Back", "Words lean back and straighten one after another.", [anim("Lean", {"skew": -40, "opacity": 0}, [reveal(2, 1.2)])])
add("Miscellaneous", "Hop", "Characters hop once in reading order.", [anim("Hop", {"position": [0, -50, 0]}, [travel(0, 1.4, 20, ROUND)])])
add("Miscellaneous", "Sink In", "Characters sink into the baseline in reading order.", [anim("Sink", {"position": [0, 60, 0], "opacity": 0}, [reveal(0, 1.4, out=True)])])
add("Miscellaneous", "Shake", "The whole line shakes hard, continuously.", [anim("Shake", {"position": [10, 4, 0]}, [], expressions={"position": "[Math.sin(time * 60) * 10, Math.cos(time * 47) * 4, 0]"})])
add("Miscellaneous", "Spread Words", "Words spread apart from the middle.", [anim("Spread", {"position": [0, 0, 0]}, [{"kind": "expression", "expressions": {"amount": "var c = (textTotal - 1) / 2; var s = (textIndex - 1 - c) * 100 / Math.max(1, c); [s, s, s]"}, "set": {"basedOn": 2}}], keys={"position": [[0, [0, 0, 0]], [1.4, [120, 0, 0]]]}, ease=True)])

# 3D Text (per-character 3D).
add("3D Text", "Flip In X", "Characters flip down into place around their horizontal axis.", [anim("Flip X", {"rotationX": 90, "opacity": 0}, [reveal(0, 1.6)])], per_char_3d=True)
add("3D Text", "Flip In Y", "Characters turn into place around their vertical axis.", [anim("Flip Y", {"rotationY": 90, "opacity": 0}, [reveal(0, 1.6)])], per_char_3d=True)
add("3D Text", "Swing Down 3D", "Characters swing down from their tops, in depth.", [anim("Swing", {"anchor": [0, -40, 0], "rotationX": -100, "opacity": 0}, [reveal(0, 1.6)])], per_char_3d=True)
add("3D Text", "Fly In From Behind", "Characters fly forward from deep behind the layer.", [anim("Fly", {"position": [0, 0, 1200], "opacity": 0}, [reveal(0, 1.8, extra={"advanced/easeHigh": 80})])], per_char_3d=True)
add("3D Text", "Fly Out Toward Camera", "Characters fly toward the viewer and vanish.", [anim("Fly", {"position": [0, 0, -1200], "opacity": 0}, [reveal(0, 1.6, out=True)])], per_char_3d=True)
add("3D Text", "Spin Y Continuous", "Every character spins around its vertical axis, continuously.", [anim("Spin", {"rotationY": 0}, [{"kind": "range"}], expressions={"rotationY": "time * 180"})], per_char_3d=True)
add("3D Text", "Cascade 3D", "Characters tumble down in depth, one after another.", [anim("Cascade", {"rotationX": 180, "position": [0, -100, -300], "opacity": 0}, [reveal(0, 2.0)])], per_char_3d=True)
add("3D Text", "Random Tumble 3D", "Characters tumble into place in depth, in random order.", [anim("Tumble", {"rotationX": 240, "rotationY": 120, "position": [0, 0, -600], "opacity": 0}, [reveal(0, 2.0, extra={"advanced/randomize": True})])], per_char_3d=True)
add("3D Text", "Depth Fade In", "Characters fade in while rising out of depth.", [anim("Depth", {"position": [0, 0, 400], "opacity": 0}, [reveal(0, 1.6)])], per_char_3d=True)
add("3D Text", "3D Wave", "A wave of rotation rolls along the characters in depth, continuously.", [anim("3D Wave", {"rotationX": 50}, [{"kind": "expression", "expressions": {"amount": "var s = Math.sin(time * 3 - textIndex * 0.5) * 100; [s, s, s]"}}])], per_char_3d=True)
add("3D Text", "Unfold Lines 3D", "Lines unfold toward the viewer, one after another.", [anim("Unfold", {"anchor": [0, 30, 0], "rotationX": 90, "opacity": 0}, [reveal(3, 1.4)])], per_char_3d=True)
add("3D Text", "Orbit Characters", "Characters orbit gently around their centres, continuously.", [anim("Orbit", {"rotationY": 30, "position": [0, 0, 60]}, [{"kind": "range"}, wiggly(0.8, 40)])], per_char_3d=True)

CATEGORY_ORDER = ["Animate In", "Animate Out", "Blurs", "Fill and Stroke", "Lights and Optical", "Mechanical", "Multi-Line", "Organic", "Rotation", "Scale", "Tracking", "3D Text", "Miscellaneous"]


def main() -> int:
    ids = [p["id"] for p in presets]
    dupes = {i for i in ids if ids.count(i) > 1}
    if dupes:
        print("duplicate preset ids:", ", ".join(sorted(dupes)), file=sys.stderr)
        return 1
    presets.sort(key=lambda p: (CATEGORY_ORDER.index(p["category"]), p["name"]))
    doc = {
        "format": "formvfx.textAnimatorPresets/1",
        "notes": "formvfx's text animation presets (Apache-2.0). Key times are seconds from the time the preset is applied; selector paths are relative to the selector.",
        "categories": CATEGORY_ORDER,
        "presets": presets,
    }
    text = json.dumps(doc, indent=1) + "\n"
    if "--check" in sys.argv:
        same = OUT.is_file() and OUT.read_text(encoding="utf-8") == text
        print(f"presets: {'in step' if same else 'out of date (run ./formvfx presets)'}")
        return 0 if same else 1
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(text, encoding="utf-8")
    counts = {c: sum(1 for p in presets if p["category"] == c) for c in CATEGORY_ORDER}
    print(f"presets: {len(presets)} text animation presets written to {OUT.relative_to(ROOT)}")
    print("  " + ", ".join(f"{c} {n}" for c, n in counts.items()))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
