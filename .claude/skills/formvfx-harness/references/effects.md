# formvfx effects

Generated from the formvfx engine by `./formvfx agents sync`; do not edit by hand.

306 effects in 21 categories. Apply with `apply_effect {layer, effect, params}` (the id or the name) or `execute effect.apply {effect, layers}`. Each parameter is listed as `id` = default (control). After applying, the parameter's property path is `effects/#N/[id]` where N is the effect's position on the layer (1 = top); `get_layer` shows the exact paths. GPU marks effects that render on the GPU when it is available; every effect also renders on the CPU.

## Contents

`3D Channel` (8) `Audio` (13) `Blur & Sharpen` (16) `Channel` (14) `Color Correction` (39) `Distort` (38) `Expression Controls` (9) `Generate` (26) `Immersive Video` (12) `Keying` (11) `Matte` (5) `Noise & Grain` (14) `Obsolete` (10) `Paint` (1) `Perspective` (10) `Simulation` (18) `Stylize` (26) `Text` (2) `Time` (8) `Transition` (17) `Utility` (9)

## 3D Channel

- **3D Channel Extract** `fx.3d.channelExtract`
  `channel` = 0 (menu Z-Depth|Object ID|Texture UV|Surface Normals|Coverage|Background RGB|Unclamped RGB|Material ID); `blackPoint` = 5000 (slider 0..10000); `whitePoint` = 0 (slider 0..10000); `antiAlias` = false (checkbox); `clamp` = true (checkbox); `invert` = false (checkbox)
- **Cryptomatte** `fx.3d.cryptomatte`
  `layer` = "" (plain); `selection` = "" (plain); `display` = 0 (menu Colors|Edges|Matte); `matteOnly` = false (checkbox); `invert` = false (checkbox)
- **Depth Matte** `fx.3d.depthMatte`
  `depth` = 0 (slider 0..10000); `feather` = 0 (slider 0..1000); `invert` = false (checkbox)
- **Depth of Field** `fx.3d.depthOfField`
  `focalPlane` = 100 (slider 0..10000); `maxRadius` = 0 (slider 0..50); `thickness` = 0 (slider 0..1000); `bias` = 50 (slider 0..100)
- **EXtractoR** `fx.3d.extractor`
  `red` = "R" (plain); `green` = "G" (plain); `blue` = "B" (plain); `alpha` = "A" (plain); `blackPoint` = 0 (slider 0..1); `whitePoint` = 1 (slider 0..1); `unmult` = false (checkbox)
- **Fog 3D** `fx.3d.fog3d`
  `fogColor` = [0.5, 0.5, 0.5, 1] (color); `start` = 0 (slider 0..10000); `end` = 1000 (slider 0..10000); `opacity` = 100 (slider 0..100); `density` = 50 (slider 0..100); `foggyBackground` = true (checkbox); `gradientLayer` = null (layer); `contribution` = 0 (slider 0..100)
- **ID Matte** `fx.3d.idMatte`
  `auxChannel` = 0 (menu Object ID|Material ID); `idSelection` = 0 (slider 0..65535); `feather` = 0 (slider 0..10); `invert` = false (checkbox); `useCoverage` = false (checkbox)
- **IDentifier** `fx.3d.identifier`
  `channel` = "" (plain); `display` = 0 (menu Colors|Luma Matte|Alpha Matte|Raw); `id` = 0 (slider 0..65535)

## Audio

- **Backwards** `fx.audio.backwards`
  `swapChannels` = false (checkbox)
- **Bass & Treble** `fx.audio.bassTreble`
  `bass` = 0 (slider -100..100); `treble` = 0 (slider -100..100)
- **Compressor** `fx.audio.compressor`
  `threshold` = -16 (slider -60..0); `ratio` = 3 (slider 1..30); `knee` = 15 (slider 0..30); `attack` = 6 (slider 0..400); `release` = 440 (slider 1..4000); `autoRelease` = true (checkbox); `makeupGain` = 0 (slider -30..30); `outputLimit` = 0 (slider -30..0)
- **Delay** `fx.audio.delay`
  `delayTime` = 500 (slider 1..10000); `delayAmount` = 50 (slider 0..100); `feedback` = 50 (slider 0..100); `dryOut` = 75 (slider 0..100); `wetOut` = 75 (slider 0..100)
- **Distortion** `fx.audio.distortion`
  `distortionType` = 0 (menu Soft Clip|Hard Clip|Saturation 1|Saturation 2|Tube|Fuzz); `drive` = 25 (slider 0..100); `gain` = 25 (slider 0..300); `mix` = 100 (slider 0..100); `volume` = 11 (slider 0..100); `bitDepth` = 24 (slider 1..24); `downsample` = 1 (slider 1..64)
- **Flange & Chorus** `fx.audio.flangeChorus`
  `voiceSeparationTime` = 3 (slider 0..100); `voices` = 1 (slider 1..8); `modulationRate` = 0.4 (slider 0..10); `modulationDepth` = 50 (slider 0..100); `voicePhaseChange` = 0 (angle); `invertPhase` = false (checkbox); `stereoVoices` = false (checkbox); `dryOut` = 50 (slider 0..100); `wetOut` = 50 (slider 0..100)
- **Gate** `fx.audio.gate`
  `threshold` = -60 (slider -60..0); `attack` = 10 (slider 0..1000); `hold` = 40 (slider 0..2500); `release` = 220 (slider 5..4000)
- **High-Low Pass** `fx.audio.highLowPass`
  `filterOptions` = 0 (menu High Pass|Low Pass); `cutoffFrequency` = 2000 (slider 20..20000); `dryOut` = 0 (slider 0..100); `wetOut` = 100 (slider 0..100)
- **Modulator** `fx.audio.modulator`
  `modulationType` = 0 (menu Sine|Triangle); `modulationRate` = 10 (slider 0..100); `modulationDepth` = 5 (slider 0..100); `amplitudeModulation` = 0 (slider 0..100)
- **Parametric EQ** `fx.audio.parametricEq`
  `band1Enable` = true (checkbox); `band1Frequency` = 200 (slider 20..20000); `band1Bandwidth` = 30 (slider 1..100); `band1BoostCut` = 0 (slider -24..24); `band2Enable` = false (checkbox); `band2Frequency` = 1000 (slider 20..20000); `band2Bandwidth` = 30 (slider 1..100); `band2BoostCut` = 0 (slider -24..24); `band3Enable` = false (checkbox); `band3Frequency` = 5000 (slider 20..20000); `band3Bandwidth` = 30 (slider 1..100); `band3BoostCut` = 0 (slider -24..24)
- **Reverb** `fx.audio.reverb`
  `reverbTime` = 100 (slider 1..5000); `diffusion` = 75 (slider 0..100); `decay` = 25 (slider 0..100); `brightness` = 10 (slider 0..100); `dryOut` = 90 (slider 0..100); `wetOut` = 10 (slider 0..100)
- **Stereo Mixer** `fx.audio.stereoMixer`
  `leftLevel` = 100 (slider 0..400); `rightLevel` = 100 (slider 0..400); `leftPan` = -100 (slider -100..100); `rightPan` = 100 (slider -100..100); `invertPhase` = false (checkbox)
- **Tone** `fx.audio.tone`
  `waveformOptions` = 0 (menu Sine|Triangle|Saw|Square|White Noise); `frequency1` = 440 (slider 0..22000); `frequency2` = 493.88 (slider 0..22000); `frequency3` = 587.33 (slider 0..22000); `frequency4` = 659.26 (slider 0..22000); `frequency5` = 783.99 (slider 0..22000); `level` = 20 (slider 0..100)

## Blur & Sharpen

- **Bilateral Blur** `fx.blur.bilateral`
  `radius` = 5 (slider 0..30); `threshold` = 20 (slider 0..100); `colorize` = true (checkbox)
- **Camera Lens Blur** `fx.blur.cameraLens`
  `radius` = 5 (slider 0..50); `shape` = 3 (menu Triangle|Square|Pentagon|Hexagon|Heptagon|Octagon|Nonagon|Decagon); `rotation` = 0 (angle); `roundness` = 0 (slider 0..100); `aspect` = 1 (slider 0.1..10); `blurMap` = null (layer); `blurMapChannel` = 0 (menu Luminance|Red|Green|Blue|Alpha); `focalDistance` = 0 (slider 0..255); `invertMap` = false (checkbox); `gain` = 0 (slider 0..100); `threshold` = 255 (slider 0..255); `saturation` = 0 (slider 0..100); `repeatEdge` = false (checkbox)
- **Camera-Shake Deblur** `fx.blur.cameraShakeDeblur`
  `duration` = 3 (slider 1..10); `method` = 0 (menu High Quality|Standard); `strength` = 100 (slider 100..200); `sensitivity` = 50 (slider 0..100)
- **CC Cross Blur** `fx.blur.ccCrossBlur`
  `radiusX` = 10 (slider 0..100); `radiusY` = 10 (slider 0..100); `mode` = 0 (menu Normal|Add|Screen|Lighten|Darken); `repeatEdge` = false (checkbox)
- **CC Radial Blur** `fx.blur.ccRadialBlur`
  `type` = 0 (menu Straight Zoom|Fading Zoom|Centered|Rotate|Rotate Fading); `amount` = 20 (slider -100..100); `quality` = 50 (slider 0..100); `center` = {"relative": [0.5, 0.5]} (point)
- **CC Radial Fast Blur** `fx.blur.ccRadialFastBlur`
  `center` = {"relative": [0.5, 0.5]} (point); `amount` = 50 (slider 0..100); `zoom` = 0 (menu Standard|Brightest|Darkest)
- **CC Vector Blur** `fx.blur.ccVectorBlur`
  `type` = 0 (menu Natural|Constant Length|Perpendicular|Direction Center|Direction Fading); `amount` = 10 (slider 0..100); `angleOffset` = 0 (angle); `ridgeSmoothness` = 10 (slider 0..100); `vectorMap` = null (layer); `property` = 0 (menu Lightness|Red|Green|Blue|Alpha); `mapSoftness` = 10 (slider 0..100)
- **Channel Blur** `fx.blur.channel`
  `red` = 0 (slider 0..50); `green` = 0 (slider 0..50); `blue` = 0 (slider 0..50); `alpha` = 0 (slider 0..50); `edge` = 0 (menu Off|Repeat Edge Pixels); `dimensions` = 0 (menu Horizontal and Vertical|Horizontal|Vertical)
- **Compound Blur** `fx.blur.compound`
  `blurLayer` = null (layer); `maxBlur` = 20 (slider 0..100); `stretchMap` = true (checkbox); `invert` = false (checkbox)
- **Directional Blur** `fx.blur.directional`
  `direction` = 0 (angle); `length` = 0 (slider 0..100)
- **Fast Box Blur** `fx.blur.box` · GPU
  `radius` = 0 (slider 0..50); `iterations` = 3 (slider 1..5); `dimensions` = 0 (menu Horizontal and Vertical|Horizontal|Vertical); `repeatEdge` = false (checkbox)
- **Gaussian Blur** `fx.blur.gaussian` · GPU
  `blurriness` = 0 (slider 0..50); `dimensions` = 0 (menu Horizontal and Vertical|Horizontal|Vertical); `repeatEdge` = false (checkbox)
- **Radial Blur** `fx.blur.radial`
  `amount` = 10 (slider 0..100); `center` = {"relative": [0.5, 0.5]} (point); `type` = 0 (menu Spin|Zoom)
- **Sharpen** `fx.blur.sharpen`
  `amount` = 0 (slider 0..100)
- **Smart Blur** `fx.blur.smart`
  `radius` = 3 (slider 0..20); `threshold` = 25 (slider 0..100); `mode` = 0 (menu Normal|Edge Only|Overlay Edge)
- **Unsharp Mask** `fx.blur.unsharpMask`
  `amount` = 50 (slider 0..500); `radius` = 1 (slider 0.1..100); `threshold` = 0 (slider 0..255)

## Channel

- **Arithmetic** `fx.channel.arithmetic`
  `operator` = 0 (menu And|Or|Xor|Add|Subtract|Difference|Min|Max|Block Above|Block Below|...); `red` = 0 (slider 0..255); `green` = 0 (slider 0..255); `blue` = 0 (slider 0..255); `clip` = true (checkbox)
- **Blend** `fx.channel.blend`
  `layer` = null (layer); `mode` = 0 (menu Crossfade|Color Only|Tint Only|Darken Only|Lighten Only); `blend` = 50 (slider 0..100); `ifSizesDiffer` = 0 (menu Center|Stretch to Fit)
- **Calculations** `fx.channel.calculations`
  `inputChannel` = 0 (menu RGBA|Gray|Red|Green|Blue|Alpha); `invertInput` = false (checkbox); `secondLayer` = null (layer); `secondChannel` = 0 (menu RGBA|Gray|Red|Green|Blue|Alpha); `secondOpacity` = 100 (slider 0..100); `invertSecond` = false (checkbox); `stretchSecond` = true (checkbox); `mode` = 0 (menu Normal|Add|Multiply|Screen|Overlay|Soft Light|Hard Light|Darken|Lighten|Difference|...); `preserveTransparency` = false (checkbox)
- **CC Composite** `fx.channel.ccComposite`
  `opacity` = 100 (slider 0..100); `mode` = 0 (menu Behind|In Front|Add|Multiply|Screen|Darken|Lighten|Difference); `rgbOnly` = false (checkbox)
- **Channel Combiner** `fx.channel.channelCombiner`
  `useSecondLayer` = false (checkbox); `sourceLayer` = null (layer); `from` = 0 (menu RGB to HLS|HLS to RGB|RGB to YUV|YUV to RGB|Red Only|Green Only|Blue Only|Alpha Only|Lightness Only|Hue Only|...); `to` = 0 (menu Red Only|Green Only|Blue Only|Alpha Only|Lightness Only|Hue Only|Saturation Only); `invert` = false (checkbox); `solidAlpha` = false (checkbox)
- **Channel Mixer** `fx.color.channelMixer`
  `rr` = 100 (slider -200..200); `rg` = 0 (slider -200..200); `rb` = 0 (slider -200..200); `gr` = 0 (slider -200..200); `gg` = 100 (slider -200..200); `gb` = 0 (slider -200..200); `br` = 0 (slider -200..200); `bg` = 0 (slider -200..200); `bb` = 100 (slider -200..200); `mono` = false (checkbox)
- **Compound Arithmetic** `fx.channel.compoundArithmetic`
  `secondSource` = null (layer); `operator` = 0 (menu Copy|Add|Subtract|Multiply|Difference|And|Or|Xor|Lighten|Darken|...); `operateOn` = 0 (menu RGB|ARGB|Alpha); `overflow` = 0 (menu Clip|Wrap|Scale); `stretch` = true (checkbox); `blend` = 0 (slider 0..100)
- **Invert** `fx.channel.invert` · GPU
  `channel` = 0 (menu RGB|Red|Green|Blue|Alpha|Luminance); `blend` = 0 (slider 0..100)
- **Minimax** `fx.channel.minimax`
  `operation` = 0 (menu Maximum|Minimum|Minimum then Maximum|Maximum then Minimum); `radius` = 0 (slider 0..50); `channel` = 0 (menu Color|Alpha and Color|Alpha|Red|Green|Blue); `direction` = 0 (menu Horizontal & Vertical|Just Horizontal|Just Vertical); `dontShrink` = false (checkbox)
- **Remove Color Matting** `fx.channel.removeColorMatting`
  `background` = [0, 0, 0, 1] (color); `clip` = true (checkbox)
- **Set Channels** `fx.channel.setChannels`
  `redLayer` = null (layer); `redFrom` = 0 (menu Red|Green|Blue|Alpha|Luminance|Hue|Lightness|Saturation|Full|Half|...); `greenLayer` = null (layer); `greenFrom` = 1 (menu Red|Green|Blue|Alpha|Luminance|Hue|Lightness|Saturation|Full|Half|...); `blueLayer` = null (layer); `blueFrom` = 2 (menu Red|Green|Blue|Alpha|Luminance|Hue|Lightness|Saturation|Full|Half|...); `alphaLayer` = null (layer); `alphaFrom` = 3 (menu Red|Green|Blue|Alpha|Luminance|Hue|Lightness|Saturation|Full|Half|...); `ifSizesDiffer` = 1 (menu Center|Stretch to Fit)
- **Set Matte** `fx.channel.setMatte`
  `takeMatteFrom` = null (layer); `useForMatte` = 3 (menu Red|Green|Blue|Alpha|Luminance|Hue|Lightness|Saturation|Full|Half|...); `invert` = false (checkbox); `stretch` = true (checkbox); `composite` = true (checkbox); `premultiply` = true (checkbox)
- **Shift Channels** `fx.channel.shiftChannels`
  `alpha` = 3 (menu Red|Green|Blue|Alpha|Luminance|Hue|Lightness|Saturation|Full|Half|...); `red` = 0 (menu Red|Green|Blue|Alpha|Luminance|Hue|Lightness|Saturation|Full|Half|...); `green` = 1 (menu Red|Green|Blue|Alpha|Luminance|Hue|Lightness|Saturation|Full|Half|...); `blue` = 2 (menu Red|Green|Blue|Alpha|Luminance|Hue|Lightness|Saturation|Full|Half|...)
- **Solid Composite** `fx.channel.solidComposite`
  `sourceOpacity` = 100 (slider 0..100); `color` = [1, 1, 1, 1] (color); `opacity` = 100 (slider 0..100); `mode` = 0 (menu Normal|Add|Multiply|Screen|Overlay|Darken|Lighten|Difference)

## Color Correction

- **Auto Color** `fx.color.autoColor`
  `smoothing` = 0 (slider 0..10); `sceneDetect` = false (checkbox); `blackClip` = 0.1 (slider 0..10); `whiteClip` = 0.1 (slider 0..10); `snapNeutral` = false (checkbox); `blend` = 0 (slider 0..100)
- **Auto Contrast** `fx.color.autoContrast`
  `smoothing` = 0 (slider 0..10); `sceneDetect` = false (checkbox); `blackClip` = 0.1 (slider 0..10); `whiteClip` = 0.1 (slider 0..10); `blend` = 0 (slider 0..100)
- **Auto Levels** `fx.color.autoLevels`
  `smoothing` = 0 (slider 0..10); `sceneDetect` = false (checkbox); `blackClip` = 0.1 (slider 0..10); `whiteClip` = 0.1 (slider 0..10); `blend` = 0 (slider 0..100)
- **Black & White** `fx.color.blackWhite`
  `reds` = 40 (slider -200..300); `yellows` = 60 (slider -200..300); `greens` = 40 (slider -200..300); `cyans` = 60 (slider -200..300); `blues` = 20 (slider -200..300); `magentas` = 80 (slider -200..300); `tint` = false (checkbox); `tintColor` = [0.89, 0.78, 0.6, 1] (color)
- **Brightness & Contrast** `fx.color.brightnessContrast` · GPU
  `brightness` = 0 (slider -150..150); `contrast` = 0 (slider -100..100)
- **Broadcast Colors** `fx.color.broadcastColors`
  `locale` = 0 (menu NTSC|PAL); `method` = 0 (menu Reduce Luminance|Reduce Saturation|Key Out Unsafe|Key Out Safe); `maxAmplitude` = 110 (slider 90..120)
- **CC Color Neutralizer** `fx.color.ccColorNeutralizer`
  `shadowsUnbalance` = [0, 0, 0, 1] (color); `midtonesUnbalance` = [0.5, 0.5, 0.5, 1] (color); `highlightsUnbalance` = [1, 1, 1, 1] (color); `pinning` = 0 (slider 0..100); `blend` = 0 (slider 0..100)
- **CC Color Offset** `fx.color.ccColorOffset`
  `red` = 0 (angle); `green` = 0 (angle); `blue` = 0 (angle); `overflow` = 0 (menu Wrap|Solarize|Polarize)
- **CC Kernel** `fx.color.ccKernel`
  `kernel` = "0 -1 0 -1 5 -1 0 -1 0" (plain); `divisor` = 1 (slider -100..100); `offset` = 0 (slider -1..1); `absolute` = false (checkbox); `blend` = 0 (slider 0..100)
- **CC Toner** `fx.color.ccToner`
  `tones` = 1 (menu Duotone|Tritone|Pentone|Solid); `highlights` = [1, 1, 1, 1] (color); `brights` = [0.85, 0.75, 0.5, 1] (color); `midtones` = [0.6, 0.45, 0.3, 1] (color); `darktones` = [0.3, 0.2, 0.1, 1] (color); `shadows` = [0, 0, 0, 1] (color); `blend` = 0 (slider 0..100)
- **Change Color** `fx.color.changeColor`
  `view` = 0 (menu Corrected Layer|Color Correction Mask); `hueTransform` = 0 (angle); `lightnessTransform` = 0 (slider -100..100); `saturationTransform` = 0 (slider -100..100); `colorToChange` = [1, 0, 0, 1] (color); `tolerance` = 15 (slider 0..100); `softness` = 0 (slider 0..100); `matchColors` = 0 (menu Using RGB|Using Hue|Using Chroma); `invert` = false (checkbox)
- **Change to Color** `fx.color.changeToColor`
  `from` = [1, 0, 0, 1] (color); `to` = [0, 0, 1, 1] (color); `change` = 0 (menu Hue|Hue & Lightness|Hue & Saturation|Hue, Lightness & Saturation); `changeBy` = 0 (menu Setting To Color|Transforming To Color); `toleranceHue` = 5 (slider 0..100); `toleranceLightness` = 50 (slider 0..100); `toleranceSaturation` = 50 (slider 0..100); `softness` = 50 (slider 0..100); `viewMatte` = false (checkbox)
- **Color Balance** `fx.color.colorBalance`
  `shadowRed` = 0 (slider -100..100); `shadowGreen` = 0 (slider -100..100); `shadowBlue` = 0 (slider -100..100); `midRed` = 0 (slider -100..100); `midGreen` = 0 (slider -100..100); `midBlue` = 0 (slider -100..100); `highRed` = 0 (slider -100..100); `highGreen` = 0 (slider -100..100); `highBlue` = 0 (slider -100..100); `preserve` = false (checkbox)
- **Color Balance (HLS)** `fx.color.colorBalanceHls`
  `hue` = 0 (angle); `lightness` = 0 (slider -100..100); `saturation` = 0 (slider -100..100)
- **Color Grade** `fx.color.grade`
  `basic/inputLut` = "" (plain); `basic/temperature` = 0 (slider -100..100); `basic/tint` = 0 (slider -100..100); `basic/exposure` = 0 (slider -5..5); `basic/contrast` = 0 (slider -100..100); `basic/highlights` = 0 (slider -100..100); `basic/shadows` = 0 (slider -100..100); `basic/whites` = 0 (slider -100..100); `basic/blacks` = 0 (slider -100..100); `basic/saturation` = 100 (slider 0..200); `creative/look` = "" (plain); `creative/lookIntensity` = 100 (slider 0..100); `creative/fadedFilm` = 0 (slider 0..100); `creative/sharpen` = 0 (slider -100..100); `creative/vibrance` = 0 (slider -100..100); `creative/saturation` = 100 (slider 0..200); `creative/shadowTint` = [0.5, 0.5, 0.5, 1] (color); `creative/highlightTint` = [0.5, 0.5, 0.5, 1] (color); `creative/tintBalance` = 0 (slider -100..100); `curves/rgb` = "0,0 1,1" (hidden); `curves/red` = "0,0 1,1" (hidden); `curves/green` = "0,0 1,1" (hidden); `curves/blue` = "0,0 1,1" (hidden); `wheels/shadows` = [0.5, 0.5, 0.5, 1] (color); `wheels/shadowLightness` = 0 (slider -100..100); `wheels/midtones` = [0.5, 0.5, 0.5, 1] (color); `wheels/midtoneLightness` = 0 (slider -100..100); `wheels/highlights` = [0.5, 0.5, 0.5, 1] (color); `wheels/highlightLightness` = 0 (slider -100..100); `hsl/enable` = false (checkbox); `hsl/hue` = 0 (angle); `hsl/hueRange` = 30 (slider 0..180); `hsl/satMin` = 10 (slider 0..100); `hsl/lumMin` = 0 (slider 0..100); `hsl/lumMax` = 100 (slider 0..100); `hsl/denoise` = 10 (slider 0..100); `hsl/showMask` = false (checkbox); `hsl/correction` = [0.5, 0.5, 0.5, 1] (color); `hsl/temperature` = 0 (slider -100..100); `hsl/saturation` = 100 (slider 0..200); `hsl/contrast` = 0 (slider -100..100); `vignette/amount` = 0 (slider -5..5); `vignette/midpoint` = 50 (slider 0..100); `vignette/roundness` = 0 (slider -100..100); `vignette/feather` = 50 (slider 0..100)
- **Color Link** `fx.color.colorLink`
  `source` = null (layer); `sample` = 0 (menu Average|Median|Brightest Values|Darkest Values|Max RGB|Min RGB|Average Alpha|Alpha Only|Max Alpha); `clip` = 0 (slider 0..100); `stencil` = false (checkbox); `opacity` = 100 (slider 0..100); `mode` = 0 (menu Normal|Multiply|Screen|Overlay|Color|Hue)
- **Color Stabilizer** `fx.color.colorStabilizer`
  `stabilize` = 0 (menu Brightness|Levels|Curves); `referenceFrame` = 0 (slider -100000..100000); `blackPoint` = {"relative": [0.2, 0.5]} (point); `midPoint` = {"relative": [0.5, 0.5]} (point); `whitePoint` = {"relative": [0.8, 0.5]} (point); `sampleSize` = 5 (slider 1..50)
- **Colorama** `fx.color.colorama`
  `input/getPhaseFrom` = 0 (menu Intensity|Red|Green|Blue|Hue|Lightness|Saturation|Value|Alpha|Zero); `input/phaseShift` = 0 (angle); `output/palette` = 0 (menu Hue Cycle|Ramp Grayscale|Fire|Ice|Ramp Red|Ramp Green|Ramp Blue|Pastel Cycle|Custom); `output/custom` = "0:1,0,0 0.5:0,1,0 1:0,0,1" (plain); `output/cycles` = 1 (slider 0..10); `output/interpolate` = true (checkbox); `modify/channels` = 0 (menu All|Red|Green|Blue|Hue|Lightness|Saturation); `modify/alpha` = false (checkbox); `blend` = 0 (slider 0..100)
- **Curves** `fx.color.curves`
  `channel` = 0 (menu RGB|Red|Green|Blue|Alpha); `rgb` = "0,0 1,1" (hidden); `red` = "0,0 1,1" (hidden); `green` = "0,0 1,1" (hidden); `blue` = "0,0 1,1" (hidden); `alpha` = "0,0 1,1" (hidden)
- **Equalize** `fx.color.equalize`
  `style` = 0 (menu RGB|Brightness|Classic Style); `amount` = 100 (slider 0..100)
- **Exposure** `fx.color.exposure` · GPU
  `exposure` = 0 (slider -5..5); `offset` = 0 (slider -0.5..0.5); `gamma` = 1 (slider 0.1..3)
- **Gamma/Pedestal/Gain** `fx.color.gammaPedestalGain`
  `blackStretch` = 0 (slider 0..4); `redGamma` = 1 (slider 0.1..4); `redPedestal` = 0 (slider -1..1); `redGain` = 1 (slider 0..4); `greenGamma` = 1 (slider 0.1..4); `greenPedestal` = 0 (slider -1..1); `greenGain` = 1 (slider 0..4); `blueGamma` = 1 (slider 0.1..4); `bluePedestal` = 0 (slider -1..1); `blueGain` = 1 (slider 0..4)
- **Hue/Saturation** `fx.color.hueSaturation`
  `hue` = 0 (angle); `saturation` = 0 (slider -100..100); `lightness` = 0 (slider -100..100); `colorize` = false (checkbox); `colorizeHue` = 0 (angle); `colorizeSaturation` = 25 (slider 0..100); `colorizeLightness` = 0 (slider -100..100)
- **Leave Color** `fx.color.leaveColor`
  `amount` = 0 (slider 0..100); `colorToLeave` = [1, 0, 0, 1] (color); `tolerance` = 15 (slider 0..100); `softness` = 0 (slider 0..100); `matchColors` = 0 (menu Using RGB|Using Hue)
- **Levels** `fx.color.levels`
  `channel` = 0 (menu RGB|Red|Green|Blue|Alpha); `inputBlack` = 0 (slider 0..255); `inputWhite` = 255 (slider 0..255); `gamma` = 1 (slider 0.1..3); `outputBlack` = 0 (slider 0..255); `outputWhite` = 255 (slider 0..255)
- **Levels (Individual Controls)** `fx.color.levelsIndividual`
  `rgb/inBlack` = 0 (slider 0..1); `rgb/inWhite` = 1 (slider 0..1); `rgb/gamma` = 1 (slider 0.1..10); `rgb/outBlack` = 0 (slider 0..1); `rgb/outWhite` = 1 (slider 0..1); `red/inBlack` = 0 (slider 0..1); `red/inWhite` = 1 (slider 0..1); `red/gamma` = 1 (slider 0.1..10); `red/outBlack` = 0 (slider 0..1); `red/outWhite` = 1 (slider 0..1); `green/inBlack` = 0 (slider 0..1); `green/inWhite` = 1 (slider 0..1); `green/gamma` = 1 (slider 0.1..10); `green/outBlack` = 0 (slider 0..1); `green/outWhite` = 1 (slider 0..1); `blue/inBlack` = 0 (slider 0..1); `blue/inWhite` = 1 (slider 0..1); `blue/gamma` = 1 (slider 0.1..10); `blue/outBlack` = 0 (slider 0..1); `blue/outWhite` = 1 (slider 0..1); `alpha/inBlack` = 0 (slider 0..1); `alpha/inWhite` = 1 (slider 0..1); `alpha/gamma` = 1 (slider 0.1..10); `alpha/outBlack` = 0 (slider 0..1); `alpha/outWhite` = 1 (slider 0..1); `clipBlack` = true (checkbox); `clipWhite` = true (checkbox)
- **OCIO CDL Transform** `fx.color.ocioCdl` · GPU
  `slope/red` = 1 (slider 0..4); `slope/green` = 1 (slider 0..4); `slope/blue` = 1 (slider 0..4); `offset/red` = 0 (slider -1..1); `offset/green` = 0 (slider -1..1); `offset/blue` = 0 (slider -1..1); `power/red` = 1 (slider 0.1..4); `power/green` = 1 (slider 0.1..4); `power/blue` = 1 (slider 0.1..4); `saturation` = 1 (slider 0..4); `style` = 0 (menu ASC CDL (clamped)|No Clamp); `direction` = 0 (menu Forward|Inverse); `file` = "" (plain); `cccId` = "" (plain)
- **OCIO Color Space Transform** `fx.color.ocioColorSpace` · GPU
  `config` = 0 (menu Project|Built-in ACES 1.3|Custom (.ocio file)); `configFile` = "" (plain); `source` = "" (plain); `destination` = "ACEScct" (plain); `inverse` = false (checkbox)
- **OCIO Display Transform** `fx.color.ocioDisplay` · GPU
  `config` = 0 (menu Project|Built-in ACES 1.3|Custom (.ocio file)); `configFile` = "" (plain); `source` = "" (plain); `display` = "" (plain); `view` = "" (plain); `looks` = "" (plain); `inverse` = false (checkbox)
- **OCIO File Transform** `fx.color.ocioFile` · GPU
  `file` = "" (plain); `cccId` = "" (plain); `interpolation` = 1 (menu Nearest|Linear|Tetrahedral|Best); `inverse` = false (checkbox)
- **OCIO Look Transform** `fx.color.ocioLook` · GPU
  `config` = 0 (menu Project|Built-in ACES 1.3|Custom (.ocio file)); `configFile` = "" (plain); `source` = "" (plain); `destination` = "" (plain); `looks` = "ACES 1.3 Reference Gamut Compression" (plain); `inverse` = false (checkbox)
- **Photo Filter** `fx.color.photoFilter`
  `filter` = 0 (menu Warming Filter (85)|Warming Filter (LBA)|Warming Filter (81)|Cooling Filter (80)|Cooling Filter (LBB)|Cooling Filter (82)|Red|Orange|Yellow|Green|...); `color` = [0.925, 0.541, 0, 1] (color); `density` = 25 (slider 0..100); `preserveLuminosity` = true (checkbox)
- **PS Arbitrary Map** `fx.color.psArbitraryMap`
  `phase` = 0 (slider 0..255); `mapAlpha` = false (checkbox); `map` = "" (hidden)
- **Selective Color** `fx.color.selectiveColor`
  `method` = 0 (menu Relative|Absolute); `reds/cyan` = 0 (slider -100..100); `reds/magenta` = 0 (slider -100..100); `reds/yellow` = 0 (slider -100..100); `reds/black` = 0 (slider -100..100); `yellows/cyan` = 0 (slider -100..100); `yellows/magenta` = 0 (slider -100..100); `yellows/yellow` = 0 (slider -100..100); `yellows/black` = 0 (slider -100..100); `greens/cyan` = 0 (slider -100..100); `greens/magenta` = 0 (slider -100..100); `greens/yellow` = 0 (slider -100..100); `greens/black` = 0 (slider -100..100); `cyans/cyan` = 0 (slider -100..100); `cyans/magenta` = 0 (slider -100..100); `cyans/yellow` = 0 (slider -100..100); `cyans/black` = 0 (slider -100..100); `blues/cyan` = 0 (slider -100..100); `blues/magenta` = 0 (slider -100..100); `blues/yellow` = 0 (slider -100..100); `blues/black` = 0 (slider -100..100); `magentas/cyan` = 0 (slider -100..100); `magentas/magenta` = 0 (slider -100..100); `magentas/yellow` = 0 (slider -100..100); `magentas/black` = 0 (slider -100..100); `whites/cyan` = 0 (slider -100..100); `whites/magenta` = 0 (slider -100..100); `whites/yellow` = 0 (slider -100..100); `whites/black` = 0 (slider -100..100); `neutrals/cyan` = 0 (slider -100..100); `neutrals/magenta` = 0 (slider -100..100); `neutrals/yellow` = 0 (slider -100..100); `neutrals/black` = 0 (slider -100..100); `blacks/cyan` = 0 (slider -100..100); `blacks/magenta` = 0 (slider -100..100); `blacks/yellow` = 0 (slider -100..100); `blacks/black` = 0 (slider -100..100)
- **Shadow/Highlight** `fx.color.shadowHighlight`
  `auto` = true (checkbox); `shadowAmount` = 50 (slider 0..100); `highlightAmount` = 0 (slider 0..100); `more/shadowTonalWidth` = 50 (slider 0..100); `more/shadowRadius` = 30 (slider 0..100); `more/highlightTonalWidth` = 50 (slider 0..100); `more/highlightRadius` = 30 (slider 0..100); `more/colorCorrection` = 20 (slider -100..100); `more/midtoneContrast` = 0 (slider -100..100); `blend` = 0 (slider 0..100)
- **Tint** `fx.color.tint` · GPU
  `black` = [0, 0, 0, 1] (color); `white` = [1, 1, 1, 1] (color); `amount` = 100 (slider 0..100)
- **Tritone** `fx.color.tritone`
  `highlights` = [1, 1, 1, 1] (color); `midtones` = [0.6, 0.47, 0.35, 1] (color); `shadows` = [0, 0, 0, 1] (color); `blend` = 0 (slider 0..100)
- **Vibrance** `fx.color.vibrance`
  `vibrance` = 0 (slider -100..100); `saturation` = 0 (slider -100..100)
- **Video Limiter** `fx.color.videoLimiter`
  `method` = 0 (menu Clip Luma and Chroma|Clip Signal); `lumaMin` = 0 (slider 0..1); `lumaMax` = 1 (slider 0..1); `chromaMax` = 0.5 (slider 0..1)

## Distort

- **Bezier Warp** `fx.distort.bezierWarp`
  `topLeft` = {"relative": [0.0, 0.0]} (point); `topLeftTangent` = {"relative": [0.3333333333333333, 0.0]} (point); `topRightTangent` = {"relative": [0.6666666666666666, 0.0]} (point); `topRight` = {"relative": [1.0, 0.0]} (point); `rightTopTangent` = {"relative": [1.0, 0.3333333333333333]} (point); `rightBottomTangent` = {"relative": [1.0, 0.6666666666666666]} (point); `bottomRight` = {"relative": [1.0, 1.0]} (point); `bottomRightTangent` = {"relative": [0.6666666666666666, 1.0]} (point); `bottomLeftTangent` = {"relative": [0.3333333333333333, 1.0]} (point); `bottomLeft` = {"relative": [0.0, 1.0]} (point); `leftBottomTangent` = {"relative": [0.0, 0.6666666666666666]} (point); `leftTopTangent` = {"relative": [0.0, 0.3333333333333333]} (point); `quality` = 8 (slider 1..10)
- **Bulge** `fx.distort.bulge`
  `hRadius` = 50 (slider 0..1000); `vRadius` = 50 (slider 0..1000); `center` = {"relative": [0.5, 0.5]} (point); `height` = 1 (slider -4..4)
- **CC Bend It** `fx.distort.ccBendIt`
  `bend` = 10 (slider -180..180); `start` = {"relative": [0.5, 0.2]} (point); `end` = {"relative": [0.5, 0.8]} (point); `prestart` = 1 (menu None|Static|Bend)
- **CC Bender** `fx.distort.ccBender`
  `amount` = 10 (slider -100..100); `style` = 0 (menu Bend|Marilyn|Sharp|Boxer); `adjust` = false (checkbox); `top` = {"relative": [0.5, 0.0]} (point); `base` = {"relative": [0.5, 1.0]} (point)
- **CC Blobbylize** `fx.distort.ccBlobbylize`
  `blobLayer` = null (layer); `property` = 4 (menu Red|Green|Blue|Alpha|Luminance); `softness` = 10 (slider 0..100); `cutAway` = 0 (slider 0..100); `lightIntensity` = 100 (slider 0..100); `lightColor` = [1, 1, 1, 1] (color); `lightDirection` = -45 (angle); `shadowAmount` = 0 (slider 0..100); `refraction` = 10 (slider 0..100)
- **CC Flo Motion** `fx.distort.ccFloMotion`
  `knot1` = {"relative": [0.3, 0.5]} (point); `amount1` = 50 (slider -100..100); `knot2` = {"relative": [0.7, 0.5]} (point); `amount2` = 50 (slider -100..100); `tileEdges` = true (checkbox); `falloff` = 1 (slider 0..10)
- **CC Griddler** `fx.distort.ccGriddler`
  `horizontalScale` = 100 (slider 0..100); `verticalScale` = 100 (slider 0..100); `tileSize` = 10 (slider 1..100); `rotation` = 0 (angle); `cut` = false (checkbox)
- **CC Lens** `fx.distort.ccLens`
  `center` = {"relative": [0.5, 0.5]} (point); `size` = 50 (slider 0..100); `convergence` = 50 (slider -100..100)
- **CC Page Turn** `fx.distort.ccPageTurn`
  `foldPosition` = {"relative": [0.8, 0.8]} (point); `foldDirection` = 225 (angle); `foldRadius` = 20 (slider 0..200); `lightDirection` = -45 (angle); `render` = 0 (menu Front & Back Page|Front Page|Back Page); `backPage` = null (layer); `backOpacity` = 100 (slider 0..100); `paperColor` = [1, 1, 1, 1] (color)
- **CC Power Pin** `fx.distort.ccPowerPin`
  `topLeft` = {"relative": [0.0, 0.0]} (point); `topRight` = {"relative": [1.0, 0.0]} (point); `bottomLeft` = {"relative": [0.0, 1.0]} (point); `bottomRight` = {"relative": [1.0, 1.0]} (point); `perspective` = 100 (slider 0..100); `expansionTop` = 0 (slider -100..100); `expansionLeft` = 0 (slider -100..100); `expansionRight` = 0 (slider -100..100); `expansionBottom` = 0 (slider -100..100)
- **CC Ripple Pulse** `fx.distort.ccRipplePulse`
  `center` = {"relative": [0.5, 0.5]} (point); `pulseLevel` = 0 (slider 0..100); `timeSpan` = 1 (slider 0..10); `amplitude` = 4 (slider -20..20); `renderBump` = false (checkbox)
- **CC Slant** `fx.distort.ccSlant`
  `slant` = 0 (slider -100..100); `stretching` = false (checkbox); `height` = 100 (slider 0..200); `floor` = 0 (slider -500..500); `setColor` = false (checkbox); `color` = [1, 1, 1, 1] (color)
- **CC Smear** `fx.distort.ccSmear`
  `from` = {"relative": [0.4, 0.5]} (point); `to` = {"relative": [0.6, 0.5]} (point); `reach` = 50 (slider 0..100); `radius` = 50 (slider 0..300)
- **CC Split** `fx.distort.ccSplit`
  `pointA` = {"relative": [0.5, 0.3]} (point); `pointB` = {"relative": [0.5, 0.7]} (point); `split` = 30 (slider 0..200)
- **CC Split 2** `fx.distort.ccSplit2`
  `pointA` = {"relative": [0.5, 0.3]} (point); `pointB` = {"relative": [0.5, 0.7]} (point); `split1` = 30 (slider 0..200); `split2` = 30 (slider 0..200)
- **CC Tiler** `fx.distort.ccTiler`
  `scale` = 50 (slider 0..100); `center` = {"relative": [0.5, 0.5]} (point); `blend` = 0 (slider 0..100)
- **Corner Pin** `fx.distort.cornerPin`
  `ul` = {"relative": [0.0, 0.0]} (point); `ur` = {"relative": [1.0, 0.0]} (point); `ll` = {"relative": [0.0, 1.0]} (point); `lr` = {"relative": [1.0, 1.0]} (point)
- **Detail-preserving Upscale** `fx.distort.detailPreservingUpscale`
  `scale` = 100 (slider 100..400); `detailReduction` = 20 (slider 0..100); `alpha` = 0 (menu Detail-preserving|Bicubic)
- **Displacement Map** `fx.distort.displacementMap`
  `mapLayer` = null (layer); `horizontal` = 0 (menu Red|Green|Blue|Alpha|Luminance|Hue|Lightness|Saturation|Full|Half|...); `maxHorizontal` = 5 (slider -100..100); `vertical` = 1 (menu Red|Green|Blue|Alpha|Luminance|Hue|Lightness|Saturation|Full|Half|...); `maxVertical` = 5 (slider -100..100); `behavior` = 1 (menu Center Map|Stretch Map to Fit|Tile Map); `wrap` = false (checkbox); `expand` = true (checkbox)
- **Liquify** `fx.distort.liquify`
  `tool` = 0 (menu Warp|Turbulence|Twirl Clockwise|Twirl Counterclockwise|Pucker|Bloat|Shift Pixels|Reflection|Clone|Reconstruction); `brushSize` = 64 (slider 1..600); `brushPressure` = 50 (slider 1..100); `viewMesh` = false (checkbox); `mesh` = "" (hidden); `meshOffset` = [0, 0] (plain); `percentage` = 100 (slider 0..100)
- **Magnify** `fx.distort.magnify`
  `shape` = 0 (menu Circle|Square); `center` = {"relative": [0.5, 0.5]} (point); `magnification` = 200 (slider 100..600); `size` = 100 (slider 10..600); `feather` = 0 (slider 0..100); `opacity` = 100 (slider 0..100); `scaling` = 0 (menu Standard|Soft|Scatter)
- **Mesh Warp** `fx.distort.meshWarp`
  `rows` = 7 (slider 1..31); `columns` = 7 (slider 1..31); `quality` = 8 (slider 1..10); `mesh` = "" (hidden)
- **Mirror** `fx.distort.mirror`
  `center` = {"relative": [0.5, 0.5]} (point); `angle` = 0 (angle)
- **Offset** `fx.distort.offset`
  `shift` = {"relative": [0.5, 0.5]} (point); `blend` = 0 (slider 0..100)
- **Optics Compensation** `fx.distort.opticsCompensation`
  `fov` = 0 (slider 0..180); `reverse` = false (checkbox); `orientation` = 0 (menu Horizontal|Vertical|Diagonal); `center` = {"relative": [0.5, 0.5]} (point)
- **Polar Coordinates** `fx.distort.polarCoordinates`
  `interpolation` = 0 (slider 0..100); `type` = 0 (menu Rect to Polar|Polar to Rect)
- **Puppet** `fx.distort.puppet`
  `onTransparent` = false (checkbox)
- **Reshape** `fx.distort.reshape`
  `sourceMask` = 0 (mask); `destinationMask` = 0 (mask); `boundaryMask` = 0 (mask); `percent` = 100 (slider 0..100); `elasticity` = 3 (menu Stiff|Less Stiff|Below Normal|Normal|Above Normal|Loose|Liquid)
- **Ripple** `fx.distort.ripple`
  `radius` = 50 (slider 0..100); `center` = {"relative": [0.5, 0.5]} (point); `conversion` = 0 (menu Asymmetric|Symmetric); `speed` = 1 (slider -10..10); `width` = 30 (slider 2..200); `height` = 30 (slider 0..200); `phase` = 0 (angle)
- **Rolling Shutter Repair** `fx.distort.rollingShutterRepair`
  `rate` = 50 (slider 0..100); `direction` = 0 (menu Top → Bottom|Bottom → Top|Left → Right|Right → Left); `method` = 0 (menu Warp|Pixel Motion)
- **Smear** `fx.distort.smear`
  `sourceMask` = 0 (mask); `boundaryMask` = 0 (mask); `offset` = [0, 0] (plain); `rotation` = 0 (angle); `scale` = 100 (slider 0..100); `percent` = 100 (slider 0..100); `elasticity` = 2 (menu Stiff|Less Stiff|Below Normal|Normal|Above Normal|Loose|Liquid)
- **Spherize** `fx.distort.spherize`
  `radius` = 0 (slider 0..500); `center` = {"relative": [0.5, 0.5]} (point)
- **Transform** `fx.distort.transform`
  `anchor` = {"relative": [0.5, 0.5]} (point); `position` = {"relative": [0.5, 0.5]} (point); `uniformScale` = true (checkbox); `scaleHeight` = 100 (slider 0..600); `scaleWidth` = 100 (slider 0..600); `skew` = 0 (slider -85..85); `skewAxis` = 0 (angle); `rotation` = 0 (angle); `opacity` = 100 (slider 0..100)
- **Turbulent Displace** `fx.distort.turbulentDisplace`
  `amount` = 50 (slider -200..200); `size` = 100 (slider 2..400); `offset` = {"relative": [0.5, 0.5]} (point); `complexity` = 1 (slider 1..10); `evolution` = 0 (angle); `pinning` = 0 (menu None|Pin All)
- **Twirl** `fx.distort.twirl`
  `angle` = 0 (angle); `radius` = 30 (slider 0..100); `center` = {"relative": [0.5, 0.5]} (point)
- **Warp** `fx.distort.warp`
  `style` = 0 (menu Arc|Arc Lower|Arc Upper|Arch|Bulge|Shell Lower|Shell Upper|Flag|Wave|Fish|...); `axis` = 0 (menu Horizontal|Vertical); `bend` = 50 (slider -100..100); `horizontal` = 0 (slider -100..100); `vertical` = 0 (slider -100..100)
- **Warp Stabilizer** `fx.distort.warpStabilizer`
  `result` = 0 (menu Smooth Motion|No Motion); `smoothness` = 50 (slider 0..100); `method` = 3 (menu Position|Position, Scale, Rotation|Perspective|Subspace Warp); `preserveScale` = false (checkbox); `framing` = 2 (menu Stabilize Only|Stabilize, Crop|Stabilize, Crop, Auto-scale|Stabilize, Synthesize Edges); `maxScale` = 150 (slider 100..200); `actionSafe` = 0 (slider 0..50); `additionalScale` = 100 (slider 50..150); `detailed` = false (checkbox); `synthRange` = 0.3 (slider 0..30); `synthFeather` = 0 (slider 0..100); `analysis` = "" (hidden)
- **Wave Warp** `fx.distort.waveWarp`
  `type` = 0 (menu Sine|Square|Triangle|Sawtooth|Circle|Semicircle|Noise); `height` = 10 (slider -100..100); `width` = 40 (slider 1..1000); `direction` = 90 (angle); `speed` = 1 (slider -10..10); `pinning` = 0 (menu None|All Edges); `phase` = 0 (angle)

## Expression Controls

- **3D Point Control** `fx.control.point3d`
  `point` = [0, 0, 0] (point3)
- **Angle Control** `fx.control.angle`
  `angle` = 0 (angle)
- **Checkbox Control** `fx.control.checkbox`
  `checkbox` = false (checkbox)
- **Color Control** `fx.control.color`
  `color` = [1, 0, 0, 1] (color)
- **Dropdown Menu Control** `fx.control.dropdown`
  `menu` = 0 (menu Item 1|Item 2|Item 3)
- **Layer Control** `fx.control.layer`
  `layer` = null (layer)
- **Point Control** `fx.control.point`
  `point` = {"relative": [0.5, 0.5]} (point)
- **Slider Control** `fx.control.slider`
  `slider` = 0 (slider 0..100)
- **Stereo 3D Controls** `fx.control.stereo3d`
  `view` = 0 (menu Anaglyph Red Cyan|Stereo Pair (Side by Side)|Left Eye|Right Eye|Balanced Red Cyan); `sceneDepth` = 3.5 (slider 0..10); `convergence` = true (checkbox); `convergenceOffset` = 0 (slider -2000..2000); `swap` = false (checkbox)

## Generate

- **4-Color Gradient** `fx.generate.fourColor`
  `p1` = {"relative": [0.1, 0.1]} (point); `c1` = [1, 1, 0, 1] (color); `p2` = {"relative": [0.9, 0.1]} (point); `c2` = [0, 1, 0, 1] (color); `p3` = {"relative": [0.1, 0.9]} (point); `c3` = [1, 0, 1, 1] (color); `p4` = {"relative": [0.9, 0.9]} (point); `c4` = [0, 0, 1, 1] (color); `blend` = 100 (slider 1..1000); `opacity` = 100 (slider 0..100); `mode` = 0 (menu None|Normal|Add|Multiply|Screen|Overlay|Soft Light|Hard Light)
- **Advanced Lightning** `fx.generate.advancedLightning`
  `type` = 0 (menu Direction|Strike|Breaking|Bouncy|Omni|Anywhere|Vertical|Two-Way Strike); `origin` = {"relative": [0.2, 0.2]} (point); `direction` = {"relative": [0.8, 0.8]} (point); `conductivity` = 10 (slider 0..100); `coreRadius` = 2 (slider 0..10); `coreOpacity` = 75 (slider 0..100); `coreColor` = [1, 1, 1, 1] (color); `glowRadius` = 30 (slider 0..100); `glowOpacity` = 50 (slider 0..100); `glowColor` = [0.25, 0.25, 1, 1] (color); `turbulence` = 1 (slider 0..10); `forking` = 25 (slider 0..100); `decay` = 0.3 (slider 0..1); `composite` = false (checkbox)
- **Audio Spectrum** `fx.generate.audioSpectrum`
  `audioLayer` = null (layer); `start` = {"relative": [0.1, 0.5]} (point); `end` = {"relative": [0.9, 0.5]} (point); `path` = 0 (mask); `startFrequency` = 20 (slider 1..10000); `endFrequency` = 1000 (slider 1..22000); `bands` = 64 (slider 1..4096); `maxHeight` = 400 (slider 0..2000); `duration` = 90 (slider 1..3000); `thickness` = 3 (slider 0..20); `softness` = 50 (slider 0..100); `inside` = [1, 1, 1, 1] (color); `outside` = [0.3, 0.6, 1, 1] (color); `display` = 0 (menu Digital|Analog lines|Analog dots); `side` = 2 (menu Side A|Side B|Side A & B); `composite` = false (checkbox)
- **Audio Waveform** `fx.generate.audioWaveform`
  `audioLayer` = null (layer); `start` = {"relative": [0.1, 0.5]} (point); `end` = {"relative": [0.9, 0.5]} (point); `path` = 0 (mask); `samples` = 200 (slider 2..2000); `maxHeight` = 100 (slider 0..500); `duration` = 200 (slider 1..10000); `thickness` = 2 (slider 0..20); `softness` = 10 (slider 0..100); `inside` = [1, 1, 1, 1] (color); `outside` = [0.3, 0.6, 1, 1] (color); `display` = 1 (menu Digital|Analog lines|Analog dots); `composite` = false (checkbox)
- **Beam** `fx.generate.beam`
  `start` = {"relative": [0.2, 0.5]} (point); `end` = {"relative": [0.8, 0.5]} (point); `length` = 30 (slider 0..100); `time` = 0 (slider 0..100); `startThickness` = 8 (slider 0..100); `endThickness` = 8 (slider 0..100); `softness` = 50 (slider 0..100); `inside` = [1, 1, 1, 1] (color); `outside` = [1, 0, 0, 1] (color); `composite` = true (checkbox)
- **CC Glue Gun** `fx.generate.ccGlueGun`
  `brushPosition` = {"relative": [0.5, 0.5]} (point); `strokeWidth` = 30 (slider 0..100); `density` = 100 (slider 0..100); `timeSpan` = 1 (slider 0..30); `reflection` = 50 (slider 0..100); `color` = [0.8, 0.9, 1, 1] (color); `lightDirection` = -45 (angle)
- **CC Light Burst 2.5** `fx.generate.ccLightBurst`
  `center` = {"relative": [0.5, 0.5]} (point); `intensity` = 100 (slider 0..500); `rayLength` = 50 (slider 0..200); `burst` = 0 (menu Straight|Fade|Center); `halfStar` = false (checkbox); `setColor` = false (checkbox); `color` = [1, 1, 1, 1] (color)
- **CC Light Rays** `fx.generate.ccLightRays`
  `intensity` = 100 (slider 0..500); `center` = {"relative": [0.5, 0.5]} (point); `radius` = 40 (slider 0..200); `warpSoftness` = 50 (slider 0..100); `shape` = 0 (menu Round|Square); `colorFromSource` = true (checkbox); `allowBrightening` = true (checkbox); `color` = [1, 1, 1, 1] (color); `mode` = 0 (menu Add|Lighten|Screen|None)
- **CC Light Sweep** `fx.generate.ccLightSweep`
  `center` = {"relative": [0.5, 0.5]} (point); `direction` = -30 (angle); `shape` = 1 (menu Linear|Smooth|Sharp); `width` = 50 (slider 0..300); `intensity` = 50 (slider 0..200); `edgeIntensity` = 100 (slider 0..200); `edgeThickness` = 1 (slider 0..10); `color` = [1, 1, 1, 1] (color); `reception` = 0 (menu Add|Composite|Cutout)
- **CC Threads** `fx.generate.ccThreads`
  `width` = 20 (slider 1..100); `height` = 20 (slider 1..100); `overlaps` = 2 (slider 1..10); `direction` = 0 (angle); `center` = {"relative": [0.5, 0.5]} (point); `coverage` = 80 (slider 0..100); `shadowing` = 50 (slider 0..100); `texture` = 20 (slider 0..100)
- **Cell Pattern** `fx.generate.cellPattern`
  `pattern` = 0 (menu Bubbles|Crystals|Plates|Static Plates|Crystallized|Pillow|Mixed Crystals|Tubular); `invert` = false (checkbox); `contrast` = 100 (slider 0..10000); `overflow` = 1 (menu Clip|Soft Clamp|Wrap Back); `disperse` = 1 (slider 0..1.5); `size` = 60 (slider 10..200); `offset` = {"relative": [0.5, 0.5]} (point); `evolution` = 0 (angle); `seed` = 0 (slider 0..10000)
- **Checkerboard** `fx.generate.checkerboard`
  `anchor` = {"relative": [0.0, 0.0]} (point); `size` = [64, 64] (pixels); `color` = [1, 1, 1, 1] (color); `opacity` = 100 (slider 0..100); `mode` = 0 (menu None|Normal|Add|Multiply|Screen|Overlay|Soft Light|Hard Light)
- **Circle** `fx.generate.circle`
  `center` = {"relative": [0.5, 0.5]} (point); `radius` = 75 (slider 0..1000); `edge` = 0 (menu None|Thickness|Edge Radius); `thickness` = 10 (slider 0..200); `featherOuter` = 0 (slider 0..200); `featherInner` = 0 (slider 0..200); `invert` = false (checkbox); `color` = [1, 1, 1, 1] (color); `opacity` = 100 (slider 0..100); `mode` = 1 (menu None|Normal|Add|Multiply|Screen|Overlay|Soft Light|Hard Light)
- **Ellipse** `fx.generate.ellipse`
  `center` = {"relative": [0.5, 0.5]} (point); `width` = 200 (slider 0..1000); `height` = 200 (slider 0..1000); `thickness` = 4 (slider 0..100); `softness` = 50 (slider 0..100); `inside` = [1, 1, 1, 1] (color); `outside` = [0, 0, 1, 1] (color); `composite` = false (checkbox)
- **Eyedropper Fill** `fx.generate.eyedropperFill`
  `samplePoint` = {"relative": [0.5, 0.5]} (point); `sampleRadius` = 0 (slider 0..50); `average` = 0 (menu Skip Empty|All|All Premultiplied|Including Alpha); `maintainAlpha` = true (checkbox); `blend` = 0 (slider 0..100)
- **Fill** `fx.generate.fill`
  `color` = [1, 0, 0, 1] (color); `invert` = false (checkbox); `opacity` = 100 (slider 0..100)
- **Fractal** `fx.generate.fractalSet`
  `set` = 0 (menu Mandelbrot|Mandelbrot Inverse|Julia|Julia Inverse); `x` = -0.75 (slider -2..2); `y` = 0 (slider -2..2); `magnification` = 0 (slider -5..30); `escape` = 100 (slider 1..1024); `juliaX` = -0.8 (slider -2..2); `juliaY` = 0.156 (slider -2..2); `palette` = 1 (menu Lightness Gradient|Hue|Rainbow); `cycleSteps` = 30 (slider 1..1000); `cycleOffset` = 0 (angle); `transparency` = false (checkbox)
- **Gradient Ramp** `fx.generate.gradientRamp`
  `start` = {"relative": [0.5, 0.0]} (point); `startColor` = [0, 0, 0, 1] (color); `end` = {"relative": [0.5, 1.0]} (point); `endColor` = [1, 1, 1, 1] (color); `shape` = 0 (menu Linear Ramp|Radial Ramp); `scatter` = 0 (slider 0..100); `blend` = 0 (slider 0..100)
- **Grid** `fx.generate.grid`
  `anchor` = {"relative": [0.0, 0.0]} (point); `size` = [64, 64] (pixels); `border` = 2 (slider 0..50); `invert` = false (checkbox); `color` = [1, 1, 1, 1] (color); `opacity` = 100 (slider 0..100); `mode` = 0 (menu None|Normal|Add|Multiply|Screen|Overlay|Soft Light|Hard Light)
- **Lens Flare** `fx.generate.lensFlare`
  `center` = {"relative": [0.3, 0.3]} (point); `brightness` = 100 (slider 0..300); `lens` = 0 (menu 50-300mm Zoom|35mm Prime|105mm Prime); `blend` = 0 (slider 0..100)
- **Paint Bucket** `fx.generate.paintBucket`
  `fillPoint` = {"relative": [0.5, 0.5]} (point); `selector` = 0 (menu Color & Alpha|Straight Color|Transparency|Opacity|Alpha Channel); `tolerance` = 10 (slider 0..100); `viewThreshold` = false (checkbox); `stroke` = 0 (menu Antialias|Feather|Spread|Choke|Stroke); `strokeWidth` = 3 (slider 0..30); `invert` = false (checkbox); `color` = [1, 0, 0, 1] (color); `opacity` = 100 (slider 0..100); `mode` = 0 (menu Normal|Add|Multiply|Screen|Behind|Fill Only)
- **Radio Waves** `fx.generate.radioWaves`
  `producer` = {"relative": [0.5, 0.5]} (point); `waveType` = 0 (menu Polygon|Mask); `sides` = 64 (slider 3..64); `star` = false (checkbox); `starDepth` = 50 (slider 0..100); `mask` = 0 (mask); `frequency` = 1 (slider 0..30); `expansion` = 5 (slider 0..50); `orientation` = 0 (angle); `spin` = 0 (angle); `velocity` = 0 (slider -50..50); `direction` = 90 (angle); `lifespan` = 10 (slider 0..100); `color` = [1, 1, 1, 1] (color); `startOpacity` = 100 (slider 0..100); `endOpacity` = 0 (slider 0..100); `startWidth` = 5 (slider 0..50); `endWidth` = 5 (slider 0..50); `fadeIn` = 0 (slider 0..10); `fadeOut` = 0 (slider 0..10)
- **Scribble** `fx.generate.scribble`
  `mask` = 0 (mask); `fill` = 0 (menu Inside|Centered Edge|Inside Edge|Outside Edge); `edgeWidth` = 10 (slider 0..100); `color` = [1, 0, 0, 1] (color); `opacity` = 100 (slider 0..100); `angle` = 45 (angle); `strokeWidth` = 3 (slider 0.1..20); `spacing` = 6 (slider 1..30); `curviness` = 50 (slider 0..100); `spacingVariation` = 25 (slider 0..100); `start` = 0 (slider 0..100); `end` = 100 (slider 0..100); `wiggle` = 1 (menu Static|Jumpy|Smooth); `wigglesPerSecond` = 5 (slider 0..30); `composite` = 1 (menu On Original Image|On Transparent|Reveal Original Image)
- **Stroke** `fx.generate.stroke`
  `path` = 0 (mask); `allMasks` = false (checkbox); `color` = [1, 1, 1, 1] (color); `brushSize` = 2 (slider 0..50); `hardness` = 75 (slider 0..100); `opacity` = 100 (slider 0..100); `start` = 0 (slider 0..100); `end` = 100 (slider 0..100); `style` = 0 (menu On Original Image|On Transparent|Reveal Original Image)
- **Vegas** `fx.generate.vegas`
  `strokeFrom` = 0 (menu Image Contours|Mask/Path); `path` = 0 (mask); `threshold` = 50 (slider 0..100); `segments` = 32 (slider 1..1000); `length` = 0.5 (slider 0..1); `rotation` = 0 (angle); `mode` = 1 (menu Transparent|Over|Under|Stencil); `color` = [1, 1, 1, 1] (color); `width` = 2 (slider 0..20); `hardness` = 0.5 (slider 0..1); `opacity` = 100 (slider 0..100)
- **Write-on** `fx.generate.writeOn`
  `brushPosition` = {"relative": [0.5, 0.5]} (point); `color` = [1, 1, 1, 1] (color); `brushSize` = 10 (slider 0..100); `hardness` = 75 (slider 0..100); `opacity` = 100 (slider 0..100); `strokeLength` = 0 (slider 0..600); `spacing` = 0.01 (slider 0.001..1); `style` = 0 (menu On Original Image|On Transparent|Reveal Original Image)

## Immersive Video

- **VR Blur** `fx.vr.blur`
  `blurriness` = 0 (slider 0..100)
- **VR Chromatic Aberrations** `fx.vr.chromatic`
  `center` = {"relative": [0.5, 0.5]} (point); `red` = 0 (slider -20..20); `green` = 0 (slider -20..20); `blue` = 0 (slider -20..20); `falloff` = 50 (slider 0..100)
- **VR Color Gradients** `fx.vr.colorGradients`
  `p1` = {"relative": [0.25, 0.25]} (point); `c1` = [1, 0.2, 0.2, 1] (color); `p2` = {"relative": [0.75, 0.25]} (point); `c2` = [0.2, 1, 0.2, 1] (color); `p3` = {"relative": [0.25, 0.75]} (point); `c3` = [0.2, 0.2, 1, 1] (color); `p4` = {"relative": [0.75, 0.75]} (point); `c4` = [1, 1, 0.2, 1] (color); `blend` = 2 (slider 0.1..10); `opacity` = 100 (slider 0..100); `mode` = 0 (menu Normal|Add|Multiply|Screen|Overlay)
- **VR Converter** `fx.vr.converter`
  `source` = 0 (menu Equirectangular|Cube Map 3x2|Fisheye|Rectilinear); `output` = 1 (menu Equirectangular|Cube Map 3x2|Fisheye|Rectilinear); `fov` = 180 (slider 90..360); `pan` = 0 (angle); `tilt` = 0 (angle); `roll` = 0 (angle)
- **VR De-Noise** `fx.vr.denoise`
  `amount` = 50 (slider 0..100); `radius` = 2 (slider 1..10)
- **VR Digital Glitch** `fx.vr.digitalGlitch`
  `master` = 50 (slider 0..100); `blocks` = 20 (slider 1..200); `displacement` = 40 (slider 0..200); `rgbSplit` = 10 (slider 0..100); `speed` = 10 (slider 0..60); `seed` = 1 (slider 0..10000)
- **VR Fractal Noise** `fx.vr.fractalNoise`
  `type` = 0 (menu Basic|Turbulent); `contrast` = 100 (slider 0..1000); `brightness` = 0 (slider -200..200); `scale` = 4 (slider 0.1..100); `complexity` = 6 (slider 1..20); `evolution` = 0 (angle); `opacity` = 100 (slider 0..100)
- **VR Glow** `fx.vr.glow`
  `threshold` = 0.6 (slider 0..1); `radius` = 30 (slider 0..200); `brightness` = 1 (slider 0..10); `tint` = false (checkbox); `tintColor` = [1, 1, 1, 1] (color)
- **VR Plane to Sphere** `fx.vr.planeToSphere`
  `plane` = null (layer); `longitude` = 0 (angle); `latitude` = 0 (angle); `rotation` = 0 (angle); `fov` = 60 (slider 1..179); `composite` = true (checkbox)
- **VR Rotate Sphere** `fx.vr.rotateSphere`
  `tilt` = 0 (angle); `pan` = 0 (angle); `roll` = 0 (angle)
- **VR Sharpen** `fx.vr.sharpen`
  `amount` = 0 (slider 0..200)
- **VR Sphere to Plane** `fx.vr.sphereToPlane`
  `fov` = 90 (slider 1..179); `pan` = 0 (angle); `tilt` = 0 (angle); `roll` = 0 (angle)

## Keying

- **Advanced Spill Suppressor** `fx.keying.advancedSpillSuppressor`
  `method` = 0 (menu Standard|Ultra); `autoKeyColor` = true (checkbox); `keyColor` = [0, 1, 0, 1] (color); `suppression` = 100 (slider 0..100); `ultra/tolerance` = 50 (slider 0..100); `ultra/desaturate` = 50 (slider 0..100); `ultra/spillRange` = 0 (slider 0..100); `ultra/spillColorCorrection` = 50 (slider 0..100); `ultra/lumaCorrection` = 50 (slider 0..100)
- **CC Simple Wire Removal** `fx.keying.simpleWireRemoval`
  `pointA` = {"relative": [0.3, 0.3]} (point); `pointB` = {"relative": [0.7, 0.7]} (point); `removalStyle` = 0 (menu Fade|Frame Offset|Displace|Displace Horizontal); `thickness` = 6 (slider 0..50); `slope` = 50 (slider 0..100); `mirrorBlend` = 0 (slider 0..100)
- **Color Difference Key** `fx.keying.colorDifferenceKey`
  `view` = 0 (menu Final Output|Matte Partial A|Matte Partial B|Matte); `keyColor` = [0, 0, 1, 1] (color); `accuracy` = 0 (menu Faster|More Accurate); `aInBlack` = 0 (slider 0..255); `aInWhite` = 255 (slider 0..255); `aGamma` = 1 (slider 0..5); `bInBlack` = 0 (slider 0..255); `bInWhite` = 255 (slider 0..255); `bGamma` = 1 (slider 0..5); `matteInBlack` = 0 (slider 0..255); `matteInWhite` = 255 (slider 0..255); `matteGamma` = 1 (slider 0..5)
- **Color Range** `fx.keying.colorRange`
  `fuzziness` = 0 (slider 0..255); `colorSpace` = 0 (menu Lab|YUV|RGB); `minL` = 0 (slider 0..255); `maxL` = 255 (slider 0..255); `minA` = 0 (slider 0..255); `maxA` = 255 (slider 0..255); `minB` = 0 (slider 0..255); `maxB` = 255 (slider 0..255)
- **Difference Matte** `fx.keying.differenceMatte`
  `view` = 0 (menu Final Output|Matte Only); `differenceLayer` = null (layer); `ifSizesDiffer` = 0 (menu Center|Stretch to Fit); `tolerance` = 15 (slider 0..100); `softness` = 0 (slider 0..100); `blur` = 0 (slider 0..20)
- **Extract** `fx.keying.extract`
  `channel` = 0 (menu Luminance|Red|Green|Blue|Alpha); `blackPoint` = 0 (slider 0..255); `whitePoint` = 255 (slider 0..255); `blackSoftness` = 0 (slider 0..255); `whiteSoftness` = 0 (slider 0..255); `invert` = false (checkbox)
- **Inner/Outer Key** `fx.keying.innerOuterKey`
  `foreground` = 0 (mask); `background` = 0 (mask); `edgeThin` = 0 (slider -100..100); `edgeFeather` = 0 (slider 0..50); `edgeThreshold` = 0 (slider 0..100); `invertExtraction` = false (checkbox); `blend` = 0 (slider 0..100)
- **Key Cleaner** `fx.keying.keyCleaner`
  `edgeRadius` = 1 (slider 0..10); `reduceChatter` = false (checkbox); `alphaContrast` = 100 (slider 0..100); `strength` = 100 (slider 0..100)
- **Keylight** `fx.keying.keylight`
  `view` = 10 (menu Source|Source Alpha|Corrected Source|Colour Correction Edges|Screen Matte|Inside Mask|Outside Mask|Combined Matte|Status|Intermediate Result|...); `unpremultiply` = true (checkbox); `screenColour` = [0, 1, 0, 1] (color); `screenGain` = 100 (slider 0..200); `screenBalance` = 50 (slider 0..100); `despillBias` = 50 (slider 0..100); `screenPreblur` = 0 (slider 0..10); `screenMatte/clipBlack` = 0 (slider 0..100); `screenMatte/clipWhite` = 100 (slider 0..100); `screenMatte/clipRollback` = 0 (slider 0..20); `screenMatte/shrinkGrow` = 0 (slider -10..10); `screenMatte/softness` = 0 (slider 0..20); `screenMatte/despotBlack` = 0 (slider 0..20); `screenMatte/despotWhite` = 0 (slider 0..20); `screenMatte/replaceMethod` = 3 (menu None|Source|Hard Colour|Soft Colour); `screenMatte/replaceColour` = [0.5, 0.5, 0.5, 1] (color); `insideMask/mask` = 0 (mask); `insideMask/softness` = 0 (slider 0..20); `insideMask/invert` = false (checkbox); `outsideMask/mask` = 0 (mask); `outsideMask/softness` = 0 (slider 0..20); `outsideMask/invert` = false (checkbox)
- **Linear Color Key** `fx.keying.linearColorKey`
  `view` = 0 (menu Final Output|Source Only|Matte Only); `keyColor` = [0, 0, 1, 1] (color); `matchColors` = 0 (menu Using RGB|Using Hue|Using Chroma); `tolerance` = 10 (slider 0..100); `softness` = 10 (slider 0..100); `operation` = 0 (menu Key Colors|Keep Colors)
- **Unmult** `fx.keying.unmult`

## Matte

- **Matte Choker** `fx.matte.matteChoker`
  `softness1` = 4 (slider 0..10); `choke1` = 75 (slider -127..127); `graySoftness1` = 10 (slider 0..100); `softness2` = 0 (slider 0..10); `choke2` = 0 (slider -127..127); `graySoftness2` = 100 (slider 0..100); `iterations` = 1 (slider 1..10)
- **Refine Hard Matte** `fx.matte.refineHardMatte`
  `edgeRadius` = 2 (slider 0..30); `viewEdgeRegion` = false (checkbox); `smooth` = 0 (slider 0..100); `feather` = 50 (slider 0..100); `contrast` = 50 (slider 0..100); `shiftEdge` = 0 (slider -100..100); `reduceChatter` = 0 (slider 0..100); `decontaminate` = true (checkbox); `decontaminationAmount` = 100 (slider 0..100)
- **Refine Soft Matte** `fx.matte.refineSoftMatte`
  `edgeRadius` = 10 (slider 0..30); `viewEdgeRegion` = false (checkbox); `smooth` = 0 (slider 0..100); `feather` = 0 (slider 0..100); `contrast` = 0 (slider 0..100); `shiftEdge` = 0 (slider -100..100); `reduceChatter` = 0 (slider 0..100); `decontaminate` = true (checkbox); `decontaminationAmount` = 100 (slider 0..100)
- **Roto Brush & Refine Edge** `fx.matte.rotoBrush`
  `model` = 0 (menu Classic|MobileSAM|SAM 2|SAM 2 Full Resolution); `renderRefineEdge` = false (checkbox); `baseRefineRadius` = 2 (slider 0..30); `rotoMatte/feather` = 0 (slider 0..100); `rotoMatte/contrast` = 0 (slider 0..100); `rotoMatte/shiftEdge` = 0 (slider -100..100); `rotoMatte/reduceChatter` = 0 (slider 0..100); `refineMatte/smooth` = 0 (slider 0..100); `refineMatte/feather` = 0 (slider 0..100); `refineMatte/contrast` = 50 (slider 0..100); `refineMatte/shiftEdge` = 0 (slider -100..100); `refineMatte/decontaminate` = true (checkbox); `refineMatte/decontaminationAmount` = 50 (slider 0..100); `strokes` = "[]" (hidden); `span` = "" (hidden); `mattes` = "" (hidden); `frozen` = "" (hidden)
- **Simple Choker** `fx.matte.simpleChoker`
  `view` = 0 (menu Final Output|Matte); `choke` = 0 (slider -100..100)

## Noise & Grain

- **Add Grain** `fx.noise.addGrain`
  `viewMode` = 2 (menu Preview|Blending Matte|Final Output); `preset` = 0 (menu None|Eastman EXR 50D|Kodak 5248|Kodak Vision 250D|Fuji Eterna 250|Kodak 5279); `intensity` = 1 (slider 0..10); `size` = 1 (slider 0.1..5); `softness` = 0 (slider 0..3); `aspect` = 1 (slider 0.5..2); `redIntensity` = 1 (slider 0..2); `greenIntensity` = 1 (slider 0..2); `blueIntensity` = 1 (slider 0..2); `monochromatic` = false (checkbox); `saturation` = 1 (slider 0..2); `shadows` = 100 (slider 0..100); `midtones` = 100 (slider 0..100); `highlights` = 100 (slider 0..100); `seed` = 0 (slider 0..10000); `animate` = true (checkbox); `animationSpeed` = 1 (slider 0..10)
- **Curl Noise** `fx.noise.curlDisplace`
  `amount` = 40 (slider 0..300); `size` = 200 (slider 10..1000); `offset` = {"relative": [0.5, 0.5]} (point); `complexity` = 3 (slider 1..8); `evolution` = 0 (angle); `speed` = 0 (slider -360..360); `seed` = 0 (slider 0..1000); `edges` = 0 (menu Repeat|Transparent)
- **Curl Noise Generator** `fx.noise.curlNoise`
  `inputNoise` = 0 (menu Internal|This Layer); `speed` = 10 (slider 0..100); `direction` = 0 (angle); `mirrorAtEdge` = false (checkbox); `scale` = 100 (slider 1..1000); `uniformScaling` = true (checkbox); `scaleWidth` = 100 (slider 1..1000); `scaleHeight` = 100 (slider 1..1000); `offset` = {"relative": [0.5, 0.5]} (point); `rotation` = 0 (angle); `evolution` = 0 (angle); `turbulenceSpeed` = 10 (slider 0..500); `noiseSpeed` = 10 (slider 0..100); `swirl` = 90 (slider -360..360); `density` = 0 (slider -100..100); `smoothness` = 50 (slider 0..100); `verticalBias` = 50 (slider 0..100); `sampleCount` = 12 (slider 3..24); `sampleRadius` = 40 (slider 0..200); `flowSoftness` = 50 (slider 0..100); `edgeDefinition` = 50 (slider 0..100); `flowFalloff` = 0 (slider 0..100); `view` = 2 (menu Input Noise|Curl Generation|Final Render); `contrast` = 100 (slider 0..400); `brightness` = 0 (slider -100..100); `clipHdr` = true (checkbox); `channel` = 0 (menu RGB|Red|Green|Blue|Alpha)
- **Dust & Scratches** `fx.noise.dustScratches`
  `radius` = 1 (slider 0..16); `threshold` = 0 (slider 0..255); `operateAlpha` = false (checkbox)
- **Fractal Noise** `fx.noise.fractalNoise`
  `fractalType` = 0 (menu Basic|Turbulent Smooth|Turbulent Basic|Turbulent Sharp|Dynamic|Max|Rocky); `noiseType` = 3 (menu Block|Linear|Soft Linear|Spline); `invert` = false (checkbox); `contrast` = 100 (slider 0..400); `brightness` = 0 (slider -200..200); `overflow` = 0 (menu Clip|Soft Clamp|Wrap Back); `rotation` = 0 (angle); `scale` = 100 (slider 20..600); `offset` = {"relative": [0.5, 0.5]} (point); `complexity` = 6 (slider 1..10); `subInfluence` = 70 (slider 0..100); `subScaling` = 56 (slider 25..100); `subRotation` = 0 (angle); `evolution` = 0 (angle); `randomSeed` = 0 (slider 0..100); `opacity` = 100 (slider 0..100); `mode` = 1 (menu None|Normal|Add|Multiply|Screen|Overlay|Soft Light|Hard Light)
- **Match Grain** `fx.noise.matchGrain`
  `noiseSource` = null (layer); `intensity` = 1 (slider 0..10); `size` = 1 (slider 0.1..5); `seed` = 0 (slider 0..10000)
- **Median** `fx.noise.median`
  `radius` = 0 (slider 0..20); `operateAlpha` = false (checkbox)
- **Median (Legacy)** `fx.noise.medianLegacy`
  `radius` = 0 (slider 0..20); `operateAlpha` = false (checkbox)
- **Noise** `fx.noise.noise`
  `amount` = 0 (slider 0..100); `color` = true (checkbox); `clip` = true (checkbox)
- **Noise Alpha** `fx.noise.noiseAlpha`
  `noise` = 0 (menu Uniform Random|Squared Random|Uniform Animation|Squared Animation); `amount` = 0 (slider 0..100); `original` = 0 (menu Add|Clamp|Scale|Edges); `overflow` = 0 (menu Clip|Wrap Back|Wrap); `seed` = 0 (slider 0..10000); `phase` = 0 (angle)
- **Noise HLS** `fx.noise.noiseHls`
  `noise` = 0 (menu Uniform|Squared|Grain); `hue` = 0 (slider 0..100); `lightness` = 0 (slider 0..100); `saturation` = 0 (slider 0..100); `grainSize` = 1 (slider 1..100); `phase` = 0 (angle)
- **Noise HLS Auto** `fx.noise.noiseHlsAuto`
  `noise` = 0 (menu Uniform|Squared|Grain); `hue` = 0 (slider 0..100); `lightness` = 0 (slider 0..100); `saturation` = 0 (slider 0..100); `grainSize` = 1 (slider 1..100); `animationSpeed` = 10 (slider 0..100)
- **Remove Grain** `fx.noise.removeGrain`
  `viewMode` = 1 (menu Preview|Final Output); `passes` = 1 (slider 0..5); `passCount` = 1 (slider 1..5); `detail` = 0.5 (slider 0..1); `temporal` = 0 (slider 0..1)
- **Turbulent Noise** `fx.noise.turbulentNoise`
  `fractalType` = 0 (menu Basic|Turbulent Smooth|Turbulent Basic|Dynamic|Max|Rocky); `noiseType` = 1 (menu Block|Linear|Soft Linear|Spline); `invert` = false (checkbox); `contrast` = 100 (slider 0..10000); `brightness` = 0 (slider -200..200); `scale` = 100 (slider 20..600); `rotation` = 0 (angle); `complexity` = 6 (slider 1..20); `evolution` = 0 (angle); `opacity` = 100 (slider 0..100)

## Obsolete

- **Basic 3D** `fx.obsolete.basic3d`
  `swivel` = 0 (angle); `tilt` = 0 (angle); `distance` = 0 (slider -200..200); `specular` = false (checkbox); `preview` = false (checkbox)
- **Basic Text** `fx.obsolete.basicText`
  `text` = "Text" (plain); `font` = "Inter" (plain); `position` = {"relative": [0.5, 0.5]} (point); `display` = 0 (menu Fill Only|Stroke Only|Fill Over Stroke|Stroke Over Fill); `fillColor` = [1, 0, 0, 1] (color); `strokeColor` = [1, 1, 1, 1] (color); `strokeWidth` = 2 (slider 0..20); `size` = 36 (slider 0..200); `tracking` = 0 (slider -20..50); `composite` = true (checkbox)
- **Color Key** `fx.obsolete.colorKey`
  `keyColor` = [0, 0, 1, 1] (color); `tolerance` = 0 (slider 0..255); `edgeThin` = 0 (slider -5..5); `edgeFeather` = 0 (slider 0..50)
- **Gaussian Blur (Legacy)** `fx.obsolete.gaussianLegacy`
  `blurriness` = 0 (slider 0..50); `dimensions` = 0 (menu Horizontal and Vertical|Horizontal|Vertical)
- **Lightning** `fx.obsolete.lightning`
  `start` = {"relative": [0.2, 0.5]} (point); `end` = {"relative": [0.8, 0.5]} (point); `segments` = 7 (slider 1..50); `amplitude` = 10 (slider 0..100); `detail` = 2 (slider 0..10); `branching` = 25 (slider 0..100); `width` = 6 (slider 0..20); `outside` = [0, 0, 1, 1] (color); `inside` = [1, 1, 1, 1] (color); `speed` = 10 (slider -10..10); `seed` = 1 (slider 0..10000)
- **Luma Key** `fx.obsolete.lumaKey`
  `keyType` = 0 (menu Key Out Brighter|Key Out Darker|Key Out Similar|Key Out Dissimilar); `threshold` = 0 (slider 0..255); `tolerance` = 0 (slider 0..255); `edgeThin` = 0 (slider -5..5); `edgeFeather` = 0 (slider 0..50)
- **Path Text** `fx.obsolete.pathText`
  `text` = "Text on a path" (plain); `font` = "Inter" (plain); `path` = 0 (mask); `fillColor` = [1, 0, 0, 1] (color); `size` = 36 (slider 0..200); `tracking` = 0 (slider -20..50); `leftMargin` = 0 (slider 0..100); `visible` = 100 (slider 0..100); `composite` = true (checkbox)
- **Reduce Interlace Flicker** `fx.obsolete.reduceInterlaceFlicker`
  `softness` = 1 (slider 0..3)
- **Spill Suppressor** `fx.obsolete.spillSuppressor`
  `colorToSuppress` = [0, 0, 1, 1] (color); `accuracy` = 0 (menu Faster|Better); `suppression` = 100 (slider 0..100)
- **Twirl (Legacy)** `fx.distort.twirlLegacy`
  `angle` = 0 (angle); `radius` = 30 (slider 0..100); `center` = {"relative": [0.5, 0.5]} (point)

## Paint

- **Paint** `fx.paint.paint`
  `paintOnTransparent` = false (checkbox)

## Perspective

- **3D Camera Tracker** `fx.perspective.cameraTracker`
  `shotType` = 0 (menu Fixed Angle of View|Variable Zoom|Specify Angle of View); `horizontalAngleOfView` = 40 (slider 10..120); `showTrackPoints` = 1 (menu 2D Source|3D Solved); `renderTrackPoints` = false (checkbox); `trackPointSize` = 100 (slider 0..300); `targetSize` = 100 (slider 10..300); `advanced/solveMethod` = 0 (menu Auto Detect|Typical|Mostly Flat Scene|Tripod Pan); `advanced/methodUsed` = "" (hidden); `advanced/averageError` = 0 (slider 0..1000); `advanced/detailedAnalysis` = false (checkbox); `tracks` = "" (hidden); `solve` = "" (hidden); `deleted` = "" (hidden); `ground` = "" (hidden); `camera` = "" (hidden)
- **3D Glasses** `fx.perspective.threeDGlasses`
  `left` = null (layer); `right` = null (layer); `convergence` = 0 (slider -1000..1000); `vertical` = 0 (slider -1000..1000); `view` = 2 (menu Stereo Pair (Side by Side)|Interlace Upper L Lower R|Red Blue LR|Red Green LR|Balanced Red Blue LR|Difference); `swap` = false (checkbox)
- **Bevel Alpha** `fx.perspective.bevelAlpha`
  `edgeThickness` = 2 (slider 0..30); `lightAngle` = -60 (angle); `lightColor` = [1, 1, 1, 1] (color); `lightIntensity` = 40 (slider 0..100)
- **Bevel Edges** `fx.perspective.bevelEdges`
  `edgeThickness` = 10 (slider 0..100); `lightAngle` = -60 (angle); `lightColor` = [1, 1, 1, 1] (color); `lightIntensity` = 40 (slider 0..100)
- **CC Cylinder** `fx.perspective.ccCylinder`
  `radius` = 50 (slider 0..100); `position` = {"relative": [0.5, 0.5]} (point); `rotationX` = 0 (angle); `rotationY` = 0 (angle); `rotationZ` = 0 (angle); `render` = 0 (menu Full|Outside|Inside); `lightDirection` = -45 (angle); `lightIntensity` = 100 (slider 0..100); `ambient` = 30 (slider 0..100)
- **CC Environment** `fx.perspective.ccEnvironment`
  `environment` = null (layer); `mapping` = 0 (menu Spherical|Probe); `horizontalOffset` = 0 (angle); `filterEnvironment` = true (checkbox)
- **CC Sphere** `fx.perspective.ccSphere`
  `rotationX` = 0 (angle); `rotationY` = 0 (angle); `rotationZ` = 0 (angle); `radius` = 100 (slider 10..1000); `offset` = {"relative": [0.5, 0.5]} (point); `render` = 0 (menu Full|Outside|Inside); `lightDirection` = -45 (angle); `lightIntensity` = 100 (slider 0..100); `ambient` = 30 (slider 0..100); `reflective` = 0 (slider 0..100)
- **CC Spotlight** `fx.perspective.ccSpotlight`
  `from` = {"relative": [0.3, 0.3]} (point); `to` = {"relative": [0.5, 0.5]} (point); `height` = 50 (slider 0..200); `coneAngle` = 25 (angle); `edgeSoftness` = 50 (slider 0..100); `color` = [1, 1, 1, 1] (color); `intensity` = 100 (slider 0..100); `render` = 1 (menu Light Only|Light|Light & Shadow)
- **Drop Shadow** `fx.perspective.dropShadow`
  `color` = [0, 0, 0, 1] (color); `opacity` = 50 (slider 0..100); `direction` = 135 (angle); `distance` = 5 (slider 0..120); `softness` = 0 (slider 0..250); `shadowOnly` = false (checkbox)
- **Radial Shadow** `fx.perspective.radialShadow`
  `color` = [0, 0, 0, 1] (color); `opacity` = 50 (slider 0..100); `lightSource` = {"relative": [0.3, 0.2]} (point); `distance` = 10 (slider 1..100); `softness` = 0 (slider 0..100); `render` = 0 (menu Regular|Glass Edge); `shadowOnly` = false (checkbox)

## Simulation

- **Card Dance** `fx.sim.cardDance`
  `rows` = 10 (slider 1..100); `columns` = 10 (slider 1..100); `backLayer` = null (layer); `gradient1` = null (layer); `zSource` = 0 (slider -200..200); `xRotation` = 0 (slider -180..180); `yRotation` = 0 (slider -180..180); `zRotation` = 0 (slider -180..180); `scale` = 0 (slider -1..1); `offset` = 0 (slider -1..1); `lightDirection` = -45 (angle); `ambient` = 25 (slider 0..100)
- **Caustics** `fx.sim.caustics`
  `bottom` = null (layer); `waterSurface` = null (layer); `waveHeight` = 0.2 (slider 0..2); `smoothing` = 5 (slider 0..30); `depth` = 0.1 (slider 0..1); `ior` = 1.2 (slider 1..3); `surfaceColor` = [1, 1, 1, 1] (color); `surfaceOpacity` = 30 (slider 0..100); `strength` = 50 (slider 0..100); `lightDirection` = -45 (angle)
- **CC Ball Action** `fx.sim.ccBallAction`
  `scatter` = 0 (slider 0..200); `axis` = 0 (menu X Axis|Y Axis|Z Axis|XY Axis|XZ Axis|YZ Axis|XYZ Axis); `rotation` = 0 (angle); `twist` = 0 (angle); `grid` = 4 (slider 1..40); `ballSize` = 100 (slider 0..200); `instability` = 0 (slider 0..360)
- **CC Bubbles** `fx.sim.ccBubbles`
  `amount` = 50 (slider 0..1000); `speed` = 0.5 (slider -5..5); `wobble` = 5 (slider 0..50); `wobbleFrequency` = 1 (slider 0..10); `size` = 6 (slider 0..30); `reflection` = 0 (menu Liquid|Metal); `shading` = 0 (menu Fade Inwards|Fade Outwards|None)
- **CC Drizzle** `fx.sim.ccDrizzle`
  `dripRate` = 4 (slider 0..100); `longevity` = 1.5 (slider 0..20); `rippling` = 2 (slider 0..20); `displacement` = 50 (slider 0..200); `rippleHeight` = 30 (slider 0..100); `spreading` = 100 (slider 0..400); `seed` = 1 (slider 0..10000); `lightDirection` = -45 (angle)
- **CC Hair** `fx.sim.ccHair`
  `length` = 20 (slider 0..200); `thickness` = 1 (slider 0..10); `weight` = 0.5 (slider -10..10); `density` = 100 (slider 0..1000); `direction` = 180 (angle); `curl` = 0.2 (slider 0..5); `opacity` = 100 (slider 0..100); `shading` = 50 (slider 0..100); `seed` = 1 (slider 0..10000)
- **CC Mr. Mercury** `fx.sim.ccMrMercury`
  `radiusX` = 50 (slider 0..400); `radiusY` = 50 (slider 0..400); `producer` = {"relative": [0.5, 0.3]} (point); `direction` = 0 (angle); `velocity` = 1 (slider 0..10); `birthRate` = 1 (slider 0..20); `longevity` = 2 (slider 0..20); `gravity` = 0.5 (slider -2..2); `blobSize` = 0.3 (slider 0..1); `lightDirection` = -45 (angle); `seed` = 1 (slider 0..10000)
- **CC Particle Systems II** `fx.sim.ccParticleSystemsII`
  `birthRate` = 2 (slider 0..50); `longevity` = 1 (slider 0..30); `producer` = {"relative": [0.5, 0.5]} (point); `radiusX` = 10 (slider 0..500); `radiusY` = 10 (slider 0..500); `animation` = 0 (menu Explosive|Direction|Cone|Vortex|Fire|Jet Sideways|Twirly); `velocity` = 1.5 (slider 0..10); `gravity` = 1 (slider -5..5); `resistance` = 0 (slider 0..10); `direction` = 0 (angle); `extra` = 1 (slider 0..5); `particleType` = 2 (menu Line|Star|Faded Sphere|Shaded Sphere|Bubble|Square|Motion Square|Textured Square); `birthSize` = 4 (slider 0..50); `deathSize` = 1 (slider 0..50); `sizeVariation` = 0 (slider 0..100); `maxOpacity` = 75 (slider 0..100); `birthColor` = [1, 1, 0, 1] (color); `deathColor` = [0.6, 0, 0, 1] (color); `transfer` = 1 (menu Composite|Add|Screen|Lighten); `seed` = 0 (slider 0..10000)
- **CC Particle World** `fx.sim.ccParticleWorld`
  `birthRate` = 2 (slider 0..50); `longevity` = 1 (slider 0..30); `producer` = {"relative": [0.5, 0.5]} (point); `radiusX` = 10 (slider 0..500); `radiusY` = 10 (slider 0..500); `animation` = 0 (menu Explosive|Direction|Cone|Vortex|Fire|Jet Sideways|Twirly); `velocity` = 1.5 (slider 0..10); `gravity` = 1 (slider -5..5); `resistance` = 0 (slider 0..10); `direction` = 0 (angle); `extra` = 1 (slider 0..5); `particleType` = 2 (menu Line|Star|Faded Sphere|Shaded Sphere|Bubble|Square|Motion Square|Textured Square); `birthSize` = 4 (slider 0..50); `deathSize` = 1 (slider 0..50); `sizeVariation` = 0 (slider 0..100); `maxOpacity` = 75 (slider 0..100); `birthColor` = [1, 1, 0, 1] (color); `deathColor` = [0.6, 0, 0, 1] (color); `transfer` = 1 (menu Composite|Add|Screen|Lighten); `seed` = 0 (slider 0..10000); `producerZ` = 0 (slider -1000..1000); `cameraRotationY` = 0 (angle); `cameraRotationX` = 0 (angle); `cameraDistance` = 2 (slider 0.1..10); `floor` = 0.5 (slider -10..10); `floorBounce` = false (checkbox)
- **CC Pixel Polly** `fx.sim.ccPixelPolly`
  `force` = 100 (slider 0..400); `gravity` = 1 (slider -5..5); `spinning` = 0 (angle); `center` = {"relative": [0.5, 0.5]} (point); `directionRandom` = 10 (slider 0..100); `speedRandom` = 10 (slider 0..100); `grid` = 20 (slider 2..100); `object` = 0 (menu Polygon|Textured Polygon|Square|Textured Square); `startTime` = 0 (slider 0..100)
- **CC Rainfall** `fx.sim.ccRainfall`
  `drops` = 5000 (slider 0..30000); `size` = 3 (slider 0..20); `depth` = 5000 (slider 0..10000); `speed` = 4000 (slider 0..10000); `wind` = 800 (slider -2000..2000); `variation` = 30 (slider 0..100); `color` = [0.7, 0.8, 1, 1] (color); `opacity` = 25 (slider 0..100); `seed` = 1 (slider 0..10000); `composite` = true (checkbox)
- **CC Scatterize** `fx.sim.ccScatterize`
  `scatter` = 0 (slider 0..300); `rightTwist` = 0 (angle); `leftTwist` = 0 (angle); `transfer` = 0 (menu Composite|Add|Screen|Lighten)
- **CC Snowfall** `fx.sim.ccSnowfall`
  `flakes` = 5000 (slider 0..30000); `size` = 3 (slider 0..20); `variation` = 50 (slider 0..100); `depth` = 5000 (slider 0..10000); `speed` = 250 (slider 0..2000); `wind` = 50 (slider -500..500); `wiggle` = 3 (slider 0..20); `color` = [1, 1, 1, 1] (color); `opacity` = 100 (slider 0..100); `seed` = 1 (slider 0..10000); `composite` = true (checkbox)
- **CC Star Burst** `fx.sim.ccStarBurst`
  `scatter` = 100 (slider 0..500); `speed` = 1 (slider -5..5); `phase` = 0 (angle); `grid` = 4 (slider 1..20); `size` = 100 (slider 0..300); `blend` = 0 (slider 0..100)
- **Foam** `fx.sim.foam`
  `producer` = {"relative": [0.5, 0.9]} (point); `producerX` = 50 (slider 0..500); `rate` = 10 (slider 0..200); `size` = 8 (slider 0..60); `sizeVariance` = 50 (slider 0..100); `lifespan` = 3 (slider 0..30); `growth` = 0.1 (slider 0..5); `speed` = 80 (slider 0..300); `wind` = 0 (slider -200..200); `wobble` = 5 (slider 0..30); `color` = [0.85, 0.9, 1, 1] (color); `opacity` = 80 (slider 0..100); `seed` = 1 (slider 0..10000)
- **Particle Playground** `fx.sim.particlePlayground`
  `cannon` = {"relative": [0.5, 0.9]} (point); `barrelRadius` = 0 (slider 0..100); `perSecond` = 20 (slider 0..1000); `direction` = 0 (angle); `spread` = 20 (slider 0..360); `velocity` = 130 (slider 0..500); `velocitySpread` = 20 (slider 0..500); `color` = [1, 0, 0, 1] (color); `radius` = 2 (slider 0..20); `gravity` = 150 (slider -500..500); `gravityDirection` = 180 (angle); `lifespan` = 3 (slider 0..30)
- **Shatter** `fx.sim.shatter`
  `view` = 0 (menu Rendered|Wireframe); `pattern` = 0 (menu Squares|Bricks|Triangles|Hexagons|Glass); `repetitions` = 10 (slider 1..200); `forcePosition` = {"relative": [0.5, 0.5]} (point); `forceRadius` = 0.4 (slider 0..2); `forceStrength` = 5 (slider -20..20); `rotationSpeed` = 0.2 (slider 0..10); `randomness` = 0.1 (slider 0..1); `viscosity` = 0.1 (slider 0..1); `gravity` = 3 (slider -10..10); `gravityDirection` = 180 (angle); `startTime` = 0 (slider 0..100); `seed` = 1 (slider 0..10000)
- **Wave World** `fx.sim.waveWorld`
  `view` = 0 (menu Height Map|Wireframe Preview); `waveSpeed` = 0.3 (slider 0..2); `damping` = 0.1 (slider 0..1); `type` = 0 (menu Ring|Line); `position` = {"relative": [0.5, 0.5]} (point); `length` = 0.1 (slider 0..1); `amplitude` = 0.2 (slider 0..1); `frequency` = 1 (slider 0..20); `phase` = 0 (angle); `brightness` = 0.5 (slider 0..1); `contrast` = 0.5 (slider 0..5)

## Stylize

- **Brush Strokes** `fx.stylize.brushStrokes`
  `angle` = 135 (angle); `size` = 2 (slider 0..5); `length` = 8 (slider 0..40); `density` = 100 (slider 0..100); `randomness` = 1 (slider 0..2); `surface` = 0 (menu Paint On Original Image|Paint On Transparent|Paint On White|Paint On Black); `blend` = 0 (slider 0..100)
- **Cartoon** `fx.stylize.cartoon`
  `render` = 0 (menu Fill & Edges|Fill|Edges); `detailRadius` = 8 (slider 0..30); `detailThreshold` = 10 (slider 0..100); `shadingSteps` = 8 (slider 1..64); `shadingSmoothness` = 70 (slider 0..100); `edgeThreshold` = 1.6 (slider 0..5); `edgeWidth` = 1.5 (slider 0..5); `edgeSoftness` = 60 (slider 0..100); `edgeOpacity` = 100 (slider 0..100)
- **CC Block Load** `fx.stylize.ccBlockLoad`
  `completion` = 100 (slider 0..100); `scanlining` = true (checkbox); `antialiasing` = true (checkbox); `startCleared` = 1 (slider 0..1)
- **CC Burn Film** `fx.stylize.ccBurnFilm`
  `burn` = 0 (slider 0..100); `center` = {"relative": [0.5, 0.5]} (point); `seed` = 1 (slider 0..10000)
- **CC Glass** `fx.stylize.ccGlass`
  `bumpMap` = null (layer); `property` = 4 (menu Red|Green|Blue|Alpha|Lightness); `height` = 50 (slider -100..100); `displacement` = 100 (slider -200..200); `softness` = 10 (slider 0..100); `lightIntensity` = 100 (slider 0..100); `lightColor` = [1, 1, 1, 1] (color); `lightDirection` = -45 (angle); `ambient` = 30 (slider 0..100)
- **CC HexTile** `fx.stylize.ccHexTile`
  `radius` = 30 (slider 1..200); `center` = {"relative": [0.5, 0.5]} (point); `rotate` = 0 (angle); `smearing` = 0 (slider 0..100)
- **CC Kaleida** `fx.stylize.ccKaleida`
  `center` = {"relative": [0.5, 0.5]} (point); `size` = 20 (slider 0..100); `mirroring` = 0 (menu Unfold|Flower|Pinwheel|Star|Diamonds|Wheel|Kaleidoscope); `rotation` = 0 (angle); `floatingCenter` = false (checkbox)
- **CC Mr. Smoothie** `fx.stylize.ccMrSmoothie`
  `property` = 4 (menu Red|Green|Blue|Alpha|Lightness|Hue|Saturation); `smoothness` = 5 (slider 0..100); `sampleA` = {"relative": [0.2, 0.5]} (point); `sampleB` = {"relative": [0.8, 0.5]} (point); `phase` = 0 (slider 0..360); `blend` = 0 (slider 0..100)
- **CC Plastic** `fx.stylize.ccPlastic`
  `bumpLayer` = null (layer); `property` = 4 (menu Red|Green|Blue|Alpha|Lightness); `height` = 25 (slider -100..100); `softness` = 10 (slider 0..100); `lightDirection` = -45 (angle); `lightIntensity` = 100 (slider 0..100); `lightColor` = [1, 1, 1, 1] (color); `ambient` = 75 (slider 0..100); `reflection` = 60 (slider 0..100); `roughness` = 0.05 (slider 0..1)
- **CC RepeTile** `fx.stylize.ccRepeTile`
  `expandRight` = 0 (slider 0..1000); `expandLeft` = 0 (slider 0..1000); `expandDown` = 0 (slider 0..1000); `expandUp` = 0 (slider 0..1000); `tiling` = 0 (menu Repeat|Checker Flip H|Checker Flip V|Checker Flip|Unfold|Twist); `blendBorders` = 0 (slider 0..100)
- **CC Threshold** `fx.channel.ccThreshold`
  `threshold` = 127 (slider 0..255); `channel` = 0 (menu Luminance|Red|Green|Blue|Alpha); `invert` = false (checkbox); `blend` = 0 (slider 0..100)
- **CC Threshold RGB** `fx.channel.ccThresholdRgb`
  `red` = 127 (slider 0..255); `green` = 127 (slider 0..255); `blue` = 127 (slider 0..255); `invertRed` = false (checkbox); `invertGreen` = false (checkbox); `invertBlue` = false (checkbox); `blend` = 0 (slider 0..100)
- **CC Vignette** `fx.stylize.ccVignette`
  `amount` = 50 (slider -100..100); `angleOfView` = 30 (slider 0..100); `center` = {"relative": [0.5, 0.5]} (point); `pinHighlights` = false (checkbox)
- **Color Emboss** `fx.stylize.colorEmboss`
  `direction` = 45 (angle); `relief` = 1 (slider 1..10); `contrast` = 100 (slider 0..200); `blend` = 0 (slider 0..100)
- **Emboss** `fx.stylize.emboss`
  `direction` = 45 (angle); `relief` = 1.5 (slider 0..10); `contrast` = 100 (slider 0..100); `blend` = 0 (slider 0..100)
- **Find Edges** `fx.stylize.findEdges`
  `invert` = false (checkbox); `blend` = 0 (slider 0..100)
- **Glow** `fx.stylize.glow`
  `basedOn` = 1 (menu Alpha Channel|Color Channels); `threshold` = 60 (slider 0..100); `radius` = 10 (slider 0..200); `intensity` = 1 (slider 0..4); `composite` = 1 (menu On Top|Behind|None); `colors` = 0 (menu Original Colors|A & B Colors); `colorA` = [1, 1, 1, 1] (color); `colorB` = [0, 0, 0, 1] (color)
- **Mosaic** `fx.stylize.mosaic`
  `horizontal` = 10 (slider 1..200); `vertical` = 10 (slider 1..200); `sharp` = false (checkbox)
- **Motion Tile** `fx.stylize.motionTile`
  `center` = {"relative": [0.5, 0.5]} (point); `tileWidth` = 100 (slider 1..100); `tileHeight` = 100 (slider 1..100); `outputWidth` = 100 (slider 1..400); `outputHeight` = 100 (slider 1..400); `mirrorEdges` = false (checkbox); `phase` = 0 (angle); `horizontalPhase` = false (checkbox)
- **Posterize** `fx.stylize.posterize`
  `level` = 6 (slider 2..32)
- **Roughen Edges** `fx.stylize.roughenEdges`
  `edgeType` = 0 (menu Roughen|Roughen Color|Cut|Spiky|Rusty|Rusty Color|Photocopy|Photocopy Color); `edgeColor` = [0.7, 0.3, 0, 1] (color); `border` = 8 (slider 0..50); `edgeSharpness` = 1 (slider 0..10); `fractalInfluence` = 1 (slider 0..1); `scale` = 100 (slider 10..300); `evolution` = 0 (angle); `seed` = 0 (slider 0..10000); `complexity` = 2 (slider 1..10)
- **Scanlines** `fx.stylize.stripes`
  `spacing` = 4 (slider 1..40); `darkness` = 40 (slider 0..100); `angle` = 0 (angle)
- **Scatter** `fx.stylize.scatter`
  `amount` = 20 (slider 0..127); `grain` = 0 (menu Both|Horizontal|Vertical); `randomize` = false (checkbox)
- **Strobe Light** `fx.stylize.strobeLight`
  `color` = [1, 1, 1, 1] (color); `blend` = 0 (slider 0..100); `duration` = 0.05 (slider 0..10); `period` = 0.1 (slider 0..10); `probability` = 0 (slider 0..100); `strobe` = 0 (menu Operates On Color Only|Makes Layer Transparent); `operator` = 0 (menu Copy|Add|Subtract|Multiply|Difference|Screen); `seed` = 0 (slider 0..10000)
- **Texturize** `fx.stylize.texturize`
  `textureLayer` = null (layer); `lightDirection` = 135 (angle); `contrast` = 1 (slider 0..2); `placement` = 0 (menu Tile Texture|Center Texture|Stretch Texture to Fit)
- **Threshold** `fx.stylize.threshold`
  `level` = 128 (slider 0..255)

## Text

- **Numbers** `fx.text.numbers`
  `font` = "Inter" (plain); `type` = 0 (menu Number|Number [Leading Zeros]|Timecode [30]|Timecode [25]|Timecode [24]|Time|Numerical Date|Short Date|Long Date|Hexadecimal); `random` = false (checkbox); `value` = 0 (slider -30000..30000); `decimals` = 2 (slider 0..10); `currentTime` = false (checkbox); `position` = {"relative": [0.5, 0.5]} (point); `display` = 0 (menu Fill Only|Stroke Only|Fill Over Stroke|Stroke Over Fill); `fillColor` = [1, 0, 0, 1] (color); `strokeColor` = [1, 1, 1, 1] (color); `strokeWidth` = 2 (slider 0..20); `size` = 36 (slider 0..200); `tracking` = 0 (slider -20..50); `composite` = true (checkbox)
- **Timecode** `fx.text.timecode`
  `displayFormat` = 0 (menu SMPTE HH:MM:SS:FF|Frames|Feet + Frames (35mm)|Feet + Frames (16mm)); `timeSource` = 0 (menu Layer Source|Composition|Custom); `frameRate` = 30 (slider 1..120); `offset` = 0 (slider -100000..100000); `prefix` = "" (plain); `position` = {"relative": [0.5, 0.9]} (point); `size` = 36 (slider 6..200); `textColor` = [1, 1, 1, 1] (color); `showBox` = true (checkbox); `boxColor` = [0, 0, 0, 1] (color); `opacity` = 100 (slider 0..100); `composite` = true (checkbox)

## Time

- **CC Force Motion Blur** `fx.time.ccForceMotionBlur`
  `samples` = 8 (slider 2..64); `override` = true (checkbox); `shutterAngle` = 180 (slider 0..3600)
- **CC Wide Time** `fx.time.ccWideTime`
  `forward` = 3 (slider 0..50); `backward` = 3 (slider 0..50); `nativeMotionBlur` = false (checkbox)
- **Echo** `fx.time.echo`
  `echoTime` = -0.033 (slider -10..10); `echoes` = 1 (slider 0..50); `startingIntensity` = 1 (slider 0..1); `decay` = 1 (slider 0..1); `operator` = 0 (menu Add|Maximum|Minimum|Screen|Composite In Back|Composite In Front|Blend)
- **Pixel Motion Blur** `fx.time.pixelMotionBlur`
  `shutterControl` = 0 (menu Automatic|Manual); `shutterAngle` = 180 (slider 0..360); `samples` = 16 (slider 2..64); `vectorDetail` = 20 (slider 0..100)
- **Posterize Time** `fx.time.posterizeTime`
  `frameRate` = 12 (slider 0.1..99)
- **Time Difference** `fx.time.timeDifference`
  `target` = null (layer); `timeOffset` = 0 (slider -100..100); `contrast` = 50 (slider 0..100); `absolute` = true (checkbox); `alpha` = 0 (menu Original|Target|Blend|Max|Full On|Lightness of Result|Max of Result|Alpha Difference|Alpha Difference Only)
- **Time Displacement** `fx.time.timeDisplacement`
  `map` = null (layer); `maxDisplacement` = 1 (slider -10..10); `resolution` = 60 (slider 1..120); `stretch` = true (checkbox)
- **Timewarp** `fx.time.timewarp`
  `method` = 1 (menu Whole Frames|Frame Mix|Pixel Motion); `adjust` = 0 (menu Speed|Source Frame); `speed` = 50 (slider 0..200); `sourceFrame` = 0 (slider -100000..100000)

## Transition

- **Block Dissolve** `fx.transition.blockDissolve`
  `completion` = 0 (slider 0..100); `blockWidth` = 1 (slider 1..100); `blockHeight` = 1 (slider 1..100); `feather` = 0 (slider 0..50); `softEdges` = true (checkbox)
- **Card Wipe** `fx.transition.cardWipe`
  `completion` = 0 (slider 0..100); `width` = 50 (slider 0..100); `backLayer` = null (layer); `rows` = 6 (slider 1..1000); `columns` = 10 (slider 1..1000); `flipAxis` = 0 (menu X|Y|Random); `flipDirection` = 0 (menu Positive|Negative|Random); `flipOrder` = 0 (menu Left to Right|Right to Left|Top to Bottom|Bottom to Top|Top Left to Bottom Right|Top Right to Bottom Left|Bottom Left to Top Right|Bottom Right to Top Left|Gradient); `gradientLayer` = null (layer); `timingRandomness` = 0 (slider 0..100); `seed` = 1 (slider 0..10000)
- **CC Glass Wipe** `fx.transition.ccGlassWipe`
  `completion` = 0 (slider 0..100); `layerToReveal` = null (layer); `gradientLayer` = null (layer); `softness` = 10 (slider 0..100); `displacement` = 10 (slider 0..100)
- **CC Grid Wipe** `fx.transition.ccGridWipe`
  `completion` = 0 (slider 0..100); `center` = {"relative": [0.5, 0.5]} (point); `rotation` = 0 (angle); `border` = 0 (slider 0..100); `tiles` = 10 (slider 1..50); `shape` = 2 (menu Doors|Radial|Rectangular); `reverse` = false (checkbox)
- **CC Image Wipe** `fx.transition.ccImageWipe`
  `completion` = 0 (slider 0..100); `border` = 10 (slider 0..100); `autoSoftness` = true (checkbox); `gradientLayer` = null (layer); `property` = 4 (menu Red|Green|Blue|Alpha|Lightness); `blur` = 0 (slider 0..100); `invert` = false (checkbox)
- **CC Jaws** `fx.transition.ccJaws`
  `completion` = 0 (slider 0..100); `center` = {"relative": [0.5, 0.5]} (point); `direction` = 0 (angle); `height` = 50 (slider 0..100); `width` = 50 (slider 0..200); `shape` = 0 (menu Spikes|RoboJaw|Block|Waves)
- **CC Light Wipe** `fx.transition.ccLightWipe`
  `completion` = 0 (slider 0..100); `center` = {"relative": [0.5, 0.5]} (point); `intensity` = 100 (slider 0..500); `shape` = 0 (menu Doors|Round|Square); `direction` = 0 (angle); `colorFromSource` = false (checkbox); `color` = [1, 1, 1, 1] (color); `reverse` = false (checkbox)
- **CC Line Sweep** `fx.transition.ccLineSweep`
  `completion` = 0 (slider 0..100); `direction` = 45 (angle); `thickness` = 20 (slider 1..100); `slant` = 25 (slider -100..100); `flip` = false (checkbox)
- **CC Radial ScaleWipe** `fx.transition.ccRadialScaleWipe`
  `completion` = 0 (slider 0..100); `center` = {"relative": [0.5, 0.5]} (point); `reverse` = false (checkbox)
- **CC Scale Wipe** `fx.transition.ccScaleWipe`
  `stretch` = 10 (slider 0..200); `center` = {"relative": [0.5, 0.5]} (point); `direction` = 0 (angle)
- **CC Twister** `fx.transition.ccTwister`
  `completion` = 0 (slider 0..100); `backside` = null (layer); `shading` = true (checkbox); `center` = {"relative": [0.5, 0.5]} (point); `axis` = 90 (angle)
- **CC WarpoMatic** `fx.transition.ccWarpoMatic`
  `completion` = 0 (slider 0..100); `layerToReveal` = null (layer); `reactor` = 0 (menu Brightness|Contrast Differences|Brightness Differences|Local Differences); `smoothness` = 10 (slider 0..100); `warpAmount` = 50 (slider -200..200); `warpDirection` = 0 (angle); `blending` = 20 (slider 0..100)
- **Gradient Wipe** `fx.transition.gradientWipe`
  `completion` = 0 (slider 0..100); `softness` = 0 (slider 0..100); `gradientLayer` = null (layer); `placement` = 1 (menu Tile Gradient|Center Gradient|Stretch Gradient to Fit); `invert` = false (checkbox)
- **Iris Wipe** `fx.transition.irisWipe`
  `center` = {"relative": [0.5, 0.5]} (point); `points` = 6 (slider 3..32); `outerRadius` = 0 (slider 0..1000); `useInner` = false (checkbox); `innerRadius` = 0 (slider 0..1000); `rotation` = 0 (angle); `feather` = 0 (slider 0..100)
- **Linear Wipe** `fx.transition.linearWipe`
  `completion` = 0 (slider 0..100); `angle` = 90 (angle); `feather` = 0 (slider 0..300)
- **Radial Wipe** `fx.transition.radialWipe`
  `completion` = 0 (slider 0..100); `start` = 0 (angle); `center` = {"relative": [0.5, 0.5]} (point); `wipe` = 0 (menu Clockwise|Counterclockwise|Both); `feather` = 0 (slider 0..300)
- **Venetian Blinds** `fx.transition.venetianBlinds`
  `completion` = 0 (slider 0..100); `direction` = 0 (angle); `width` = 10 (slider 1..200); `feather` = 0 (slider 0..50)

## Utility

- **Apply Color LUT** `fx.utility.applyColorLut`
  `path` = "" (plain); `intensity` = 100 (slider 0..100)
- **CC Overbrights** `fx.utility.ccOverbrights`
  `channel` = 0 (menu RGB|Red|Green|Blue|Alpha|Luminance); `clipColor` = [1, 0, 1, 1] (color); `show` = 0 (menu Overbrights|Underblacks|Both)
- **Cineon Converter** `fx.utility.cineonConverter`
  `conversion` = 1 (menu Linear to Log|Log to Linear|Log to Log); `blackPoint` = 95 (slider 0..1023); `internalBlack` = 0 (slider 0..1); `whitePoint` = 685 (slider 0..1023); `internalWhite` = 1 (slider 0..1); `gamma` = 1.7 (slider 0.1..5); `rolloff` = 20 (slider 0..100)
- **Color Profile Converter** `fx.utility.colorProfileConverter`
  `input` = 0 (menu sRGB|Linear sRGB|Rec. 709|Display P3|Rec. 2020|opRGB (1998)); `output` = 1 (menu sRGB|Linear sRGB|Rec. 709|Display P3|Rec. 2020|opRGB (1998)); `intent` = 1 (menu Perceptual|Relative Colorimetric|Saturation|Absolute Colorimetric)
- **Face Measurements** `fx.face.measurements`
  `headPositionX` = 0 (slider -100000..100000); `headPositionY` = 0 (slider -100000..100000); `headScale` = 0 (slider -100000..100000); `headTilt` = 0 (slider -100000..100000); `leftEyeOpenness` = 0 (slider -100000..100000); `rightEyeOpenness` = 0 (slider -100000..100000); `leftEyebrowRaise` = 0 (slider -100000..100000); `rightEyebrowRaise` = 0 (slider -100000..100000); `mouthOpenness` = 0 (slider -100000..100000); `mouthWidth` = 0 (slider -100000..100000)
- **Face Track Points** `fx.face.trackPoints`
  `leftEyebrowInner` = {"relative": [0.5, 0.5]} (point); `leftEyebrowMiddle` = {"relative": [0.5, 0.5]} (point); `leftEyebrowOuter` = {"relative": [0.5, 0.5]} (point); `rightEyebrowInner` = {"relative": [0.5, 0.5]} (point); `rightEyebrowMiddle` = {"relative": [0.5, 0.5]} (point); `rightEyebrowOuter` = {"relative": [0.5, 0.5]} (point); `leftEyeInner` = {"relative": [0.5, 0.5]} (point); `leftEyeOuter` = {"relative": [0.5, 0.5]} (point); `leftEyeTop` = {"relative": [0.5, 0.5]} (point); `leftEyeBottom` = {"relative": [0.5, 0.5]} (point); `leftPupil` = {"relative": [0.5, 0.5]} (point); `rightEyeInner` = {"relative": [0.5, 0.5]} (point); `rightEyeOuter` = {"relative": [0.5, 0.5]} (point); `rightEyeTop` = {"relative": [0.5, 0.5]} (point); `rightEyeBottom` = {"relative": [0.5, 0.5]} (point); `rightPupil` = {"relative": [0.5, 0.5]} (point); `noseTip` = {"relative": [0.5, 0.5]} (point); `leftNostril` = {"relative": [0.5, 0.5]} (point); `rightNostril` = {"relative": [0.5, 0.5]} (point); `mouthLeft` = {"relative": [0.5, 0.5]} (point); `mouthRight` = {"relative": [0.5, 0.5]} (point); `mouthTop` = {"relative": [0.5, 0.5]} (point); `mouthBottom` = {"relative": [0.5, 0.5]} (point)
- **Grow Bounds** `fx.utility.growBounds`
  `pixels` = 0 (slider 0..500)
- **HDR Compander** `fx.utility.hdrCompander`
  `mode` = 0 (menu Compress Range|Expand Range); `gain` = 1 (slider 0.1..100); `gamma` = 1 (slider 0.1..10)
- **HDR Highlight Compression** `fx.utility.hdrHighlightCompression`
  `amount` = 100 (slider 0..100)
