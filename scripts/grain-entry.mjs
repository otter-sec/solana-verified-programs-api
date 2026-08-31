// Bundle entry for verify.osec.io's background. Ports the grain-bg mount from
// osec.io's Layout.astro, minus the Astro view-transition lifecycle.
//
// Build from this repo, resolving the dependency from the sibling osec.io
// checkout (the design source of truth):
//   NODE_PATH=../osec.io/node_modules bun build scripts/grain-entry.mjs \
//     --minify --format=iife --target=browser --outfile=assets/grain.js
import {
  getShaderColorFromString,
  getShaderNoiseTexture,
  GrainGradientShapes,
  grainGradientFragmentShader,
  ShaderFitOptions,
  ShaderMount,
} from "@paper-design/shaders"

const PALETTES = {
  dark: { back: "#111113", colors: ["#191919"] },
  light: { back: "#fcfcfd", colors: ["#EDEEF0"] },
}

const theme = () =>
  document.documentElement.dataset.theme ??
  (matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light")

const themeUniforms = (name) => ({
  u_colorBack: getShaderColorFromString(PALETTES[name].back),
  u_colors: PALETTES[name].colors.map(getShaderColorFromString),
  u_colorsCount: PALETTES[name].colors.length,
})

const GRAIN_MAX_PIXELS = 1920 * 1080 * 2

// osec.io awaits `noise.decode()` here. That promise can hang indefinitely in
// renderers that defer image decoding for a page they are not painting, and a
// hung await means the background silently never appears. `load` is the signal
// that actually matters -- once it fires the bitmap is valid for texImage2D.
function loaded(image) {
  if (image.complete && image.naturalWidth > 0) return Promise.resolve()
  return new Promise((resolve, reject) => {
    image.addEventListener("load", resolve, { once: true })
    image.addEventListener(
      "error",
      () => reject(new Error("grain noise texture failed to load")),
      { once: true },
    )
  })
}

async function mount() {
  const host = document.querySelector("grain-bg")
  const noise = getShaderNoiseTexture()
  if (!host || !noise) return

  await loaded(noise)

  let activeTheme = theme()

  const shader = new ShaderMount(
    host,
    grainGradientFragmentShader,
    {
      u_noiseTexture: noise,
      ...themeUniforms(activeTheme),
      u_softness: 0.66,
      u_intensity: 1,
      u_noise: 0.5,
      u_shape: GrainGradientShapes.truchet,
      u_fit: ShaderFitOptions.cover,
      u_scale: 1.4,
      u_rotation: 12,
      u_offsetX: 0,
      u_offsetY: 0,
      u_originX: 0.5,
      u_originY: 0.5,
      u_worldWidth: 1920,
      u_worldHeight: 1080,
    },
    undefined,
    0,
    8000,
    1,
    GRAIN_MAX_PIXELS,
  )

  const sync = () => {
    const nextTheme = theme()
    if (nextTheme === activeTheme) return
    activeTheme = nextTheme
    shader.setUniforms(themeUniforms(activeTheme))
  }
  new MutationObserver(sync).observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["data-theme"],
  })
  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", sync)
}

// The page is fully readable without the background, so a failure here is not
// fatal -- but it should not be invisible either.
mount().catch((error) => console.error("grain background failed to mount", error))
