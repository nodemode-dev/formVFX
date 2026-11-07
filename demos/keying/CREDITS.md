# Keying demo credits

Each source page was opened and read before the file was downloaded; the licence column quotes what the page states for that file. Pexels files are free to use under the Pexels License (https://www.pexels.com/license/).

| File in the demo | Title | Author | Source page | Licence on the page | What was changed |
| --- | --- | --- | --- | --- | --- |
| `assets/media/phone-screen.js` (a data pack holding `phone-screen-green.webm`) | Hand Holding Cellphone with Green Screen | Towfiqu barbhuiya | https://www.pexels.com/video/hand-holding-cellphone-with-green-screen-13863534/ | Free (Pexels License) | 3840x2160 29.97 fps source (the 2560x1440 file) scaled to 1280x720, 30 fps, the whole 8 s, re-encoded to VP9 with a keyframe every 12 frames, no sound |
| `../../demo/assets/media/dusk-shore.js` (referenced by path, not copied) | Aerial Shot of an Ocean During Sunset | Lachlan Ross | https://www.pexels.com/video/aerial-shot-of-an-ocean-during-sunset-5967989/ | Free (Pexels License) | see `demo/CREDITS.md`: 1280x720 VP9, 24 fps, sound removed. Here it plays inside the phone screen |

Data files made from the footage above, with no third-party content:

| File | What it is |
| --- | --- |
| `assets/media/roto-paths.js` | The outline of the hand and phone, traced from `phone-screen-green.webm` with OpenCV (colour thresholds, largest region, outline resampled to 100 points by arc length) at 11 frames, 24 frames apart. It becomes the keyframed Mask Path of the Roto Isolate composition. |
| `assets/media/track-corner.js` | The 240 positions this demo's own tracker produced for the phone's top right corner. They are re-applied at start with the tracker's Apply, for the tag, the screen content and the stabilized layer. |

The demo's interface, icons and fonts are the application's own (see `NOTICE.md`).
