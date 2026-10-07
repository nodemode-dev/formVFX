---
name: harness-director
description: Entry point for a whole formvfx piece. Use for a brief such as a title sequence, an explainer, a promo, a social video, a logo sting or any request that spans several compositions, layers and effects; it turns the brief into beats, orders the build, keeps a ledger and decides when the piece is done.
---

# harness-director

A motion piece is built the way a designer builds it: decide the frame, block the beats, build each beat, time it, polish it, render it. The tools make every step fast; the order keeps the work from being redone.

## 1. Intake

Settle these before the first command. Ask in plain prose only for what the request leaves open and a wrong guess would waste work; otherwise choose sensible values and say which.

- **Destination**: 1920 x 1080 for video, 1080 x 1920 for vertical social, 1080 x 1080 square, 3840 x 2160 for 4K. Frame rate 30 by default, 24 for a filmic look, 60 for UI or game capture.
- **Length** in seconds, and whether it must loop.
- **Content**: the words, the logo or footage files (absolute paths), the colours.
- **Delivery**: format (MP4 H.264 unless they say ProRes, GIF, PNG sequence or Lottie) and where the file goes.

## 2. Beats

Write the piece as beats with times before building. A beat is one idea on screen.

```text
0.0 - 0.6   background fades up, accent line draws on from the left
0.4 - 1.4   title rises 40 px into place with easy ease, characters fade in left to right
1.6 - 3.2   subtitle types on
3.2 - 4.0   hold
4.0 - 4.6   everything scales to 96% and fades out
```

Timing that reads well: entrances 0.4 to 0.8 s, staggers 0.03 to 0.08 s per element, holds of at least 1.5 s for text a viewer must read, exits faster than entrances.

## 3. Build order

1. The composition at the destination size, frame rate and length (`comp.new`, with a background colour).
2. Static layout first: backgrounds, plates, text, logos, placed and styled with no animation. `render_frame` once the layout is in, and fix composition problems now, while nothing is keyed.
3. Structure: nulls for groups that move together (`layer.setParent`), precompositions for pieces that repeat or need their own time (`layer.precompose`).
4. Animation, beat by beat, with `set_keyframes`. `contact_sheet` across each beat after keying it.
5. Effects and finishing: glows, blurs, grades, motion blur (`layer.setSwitch {switch: motionBlur}` plus `comp.setSwitch {switch: motionBlur}`).
6. A full `contact_sheet` of the whole piece, then the render.

Each numbered step is usually one or a few `batch` calls, so each is one undo step the person can review.

## 4. Ledger

Keep a short ledger in your replies on long jobs so the person, and you after a long context, know where things stand:

```text
comp  Promo (id 158) 1920x1080 30 fps 6 s
done  background, title block, logo
next  subtitle type-on, exit
check contact_sheet 0-6 s after the exit
```

## 5. Done means

- Every beat was looked at with `render_frame` or `contact_sheet`, and what was seen matches the brief.
- `execute expr.errors` reports no expression errors when expressions are used.
- The render finished (`render` with `wait: true` returns the output path) and the file exists at that path.
- The project is saved if the person wants to keep editing (`project {action: saveAs, path}`).
- The reply says what was made, where the file is, and anything that differs from the brief.

## When to load the others

Load the specialist sub-skill for each part (the master routing table lists them). For a deliverable that matches a known pattern, start from `recipes`, then adjust.
