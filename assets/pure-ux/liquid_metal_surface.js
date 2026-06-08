window.LiquidMetalSurface = (function() {
  'use strict';

  const DEFAULT_SHADER_CONTROLS = Object.freeze({
    repetitionPct: 200,
    contourPct: 180,
    distortionPct: 200,
    softnessPct: 0.67,
    angleDeg: 0,
    redShiftPct: 1.5,
    blueShiftPct: 1.95,
    speedPct: 50,
    scalePct: 100
  });

  function pctToUnit(value) {
    return value / 100;
  }

  function degToRad(value) {
    return value * Math.PI / 180;
  }

  function createShaderParamsFromControls(controls) {
    return {
      repetition: pctToUnit(controls.repetitionPct),
      contour: pctToUnit(controls.contourPct),
      distortion: pctToUnit(controls.distortionPct),
      softness: pctToUnit(controls.softnessPct),
      angle: degToRad(controls.angleDeg),
      redShift: pctToUnit(controls.redShiftPct),
      blueShift: pctToUnit(controls.blueShiftPct),
      scale: pctToUnit(controls.scalePct)
    };
  }

  const DEFAULT_SHADER_SPEED = pctToUnit(DEFAULT_SHADER_CONTROLS.speedPct);
  const DEFAULT_SHADER_PARAMS = Object.freeze({
    ...createShaderParamsFromControls(DEFAULT_SHADER_CONTROLS)
  });

  function readFiniteNumber(value, fallback) {
    const numeric = Number(value);
    return Number.isFinite(numeric) ? numeric : fallback;
  }

  function readOptionNumber(options, aliases, fallback) {
    const sources = [];
    if (options && typeof options.params === 'object' && options.params) {
      sources.push(options.params);
    }
    if (options && typeof options === 'object') {
      sources.push(options);
    }

    for (const source of sources) {
      for (const alias of aliases) {
        if (Object.prototype.hasOwnProperty.call(source, alias)) {
          return readFiniteNumber(source[alias], fallback);
        }
      }
    }

    return fallback;
  }

  function normalizeShaderParams(options, base = DEFAULT_SHADER_PARAMS) {
    return {
      repetition: Math.max(0.01, readOptionNumber(options, ['repetition'], base.repetition)),
      contour: Math.max(0.01, readOptionNumber(options, ['contour'], base.contour)),
      distortion: Math.max(0.0, readOptionNumber(options, ['distortion'], base.distortion)),
      softness: Math.max(0.0001, readOptionNumber(options, ['softness'], base.softness)),
      angle: readOptionNumber(options, ['angle'], base.angle),
      redShift: readOptionNumber(options, ['redShift', 'redshift'], base.redShift),
      blueShift: readOptionNumber(options, ['blueShift', 'blueshift'], base.blueShift),
      scale: Math.max(0.01, readOptionNumber(options, ['scale'], base.scale))
    };
  }

  class SurfaceShader {
    constructor(element, options = {}) {
      this.element = element;
      this.animated = true;
      this.speed = DEFAULT_SHADER_SPEED;
      this.variant = 0.0;
      this.params = { ...DEFAULT_SHADER_PARAMS };
      this.disposed = false;
      this.canvas = null;
      this.gl = null;
      this.program = null;
      this.rafId = null;
      this.resizeRafId = null;
      this.resizeObserver = null;
      this.currentFrame = 0;
      this.lastTime = 0;
      this.borderRadius = 0;
      this.applyOptions(options);

      this.initCanvas();
      if (!this.gl) {
        return;
      }
      this.initShader();
      this.start();
    }

    updateShapeMetrics(rect) {
      const computedStyle = window.getComputedStyle(this.element);
      const borderRadiusStr = computedStyle.borderRadius || '0px';
      const borderRadiusValues = borderRadiusStr.split(' ').map((value) => parseFloat(value) || 0);
      const rawBorderRadius = borderRadiusValues[0];
      const minDimension = Math.min(rect.width, rect.height);
      this.borderRadius = Math.min(rawBorderRadius, minDimension * 0.5);
    }

    syncCanvasSize() {
      if (!this.canvas) {
        return;
      }

      const rect = this.element.getBoundingClientRect();
      this.updateShapeMetrics(rect);

      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      const nextWidth = Math.max(1, Math.round(rect.width * dpr));
      const nextHeight = Math.max(1, Math.round(rect.height * dpr));

      if (this.canvas.width !== nextWidth || this.canvas.height !== nextHeight) {
        this.canvas.width = nextWidth;
        this.canvas.height = nextHeight;
      }
    }

    queueResize = () => {
      if (this.resizeRafId) {
        cancelAnimationFrame(this.resizeRafId);
      }
      this.resizeRafId = requestAnimationFrame(() => {
        this.resizeRafId = null;
        this.syncCanvasSize();
      });
    };

    initCanvas() {
      const position = window.getComputedStyle(this.element).position;
      if (position === 'static') {
        this.element.style.position = 'relative';
      }

      const rect = this.element.getBoundingClientRect();
      this.updateShapeMetrics(rect);

      this.canvas = document.createElement('canvas');
      this.canvas.className = 'liquid-metal-surface-canvas';
      this.canvas.style.cssText = [
        'position:absolute',
        'top:0',
        'left:0',
        'width:100%',
        'height:100%',
        'pointer-events:none',
        'z-index:0'
      ].join(';');

      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      this.canvas.width = Math.max(1, Math.round(rect.width * dpr));
      this.canvas.height = Math.max(1, Math.round(rect.height * dpr));

      if (this.element.firstChild) {
        this.element.insertBefore(this.canvas, this.element.firstChild);
      } else {
        this.element.appendChild(this.canvas);
      }

      this.gl = this.canvas.getContext('webgl2', {
        alpha: true,
        premultipliedAlpha: false,
        antialias: true,
        desynchronized: true
      });

      if (!this.gl) {
        if (this.canvas.parentNode) {
          this.canvas.parentNode.removeChild(this.canvas);
        }
        this.canvas = null;
        return;
      }

      this.syncCanvasSize();
      window.addEventListener('resize', this.queueResize, { passive: true });
      if (typeof ResizeObserver === 'function') {
        this.resizeObserver = new ResizeObserver(() => {
          this.queueResize();
        });
        this.resizeObserver.observe(this.element);
      }
    }

    applyOptions(options = {}) {
      this.animated = options.animated !== false;
      this.speed = Math.max(0.0, readOptionNumber(options, ['speed'], this.speed));
      if (Object.prototype.hasOwnProperty.call(options, 'variant')) {
        this.variant = options.variant === 'experimental' ? 1.0 : 0.0;
      }
      this.params = normalizeShaderParams(options, this.params);
    }

    initShader() {
      const gl = this.gl;

      const vertexShader = `#version 300 es
        precision highp float;
        in vec4 a_position;
        out vec2 v_uv;
        void main() {
          gl_Position = a_position;
          v_uv = a_position.xy;
        }
      `;

      const fragmentShader = `#version 300 es
        precision mediump float;
        uniform vec2 u_resolution;
        uniform float u_time;
        uniform float u_radius;
        uniform float u_variant;
        uniform float u_repetition;
        uniform float u_contour;
        uniform float u_distortion;
        uniform float u_softness;
        uniform float u_angle;
        uniform float u_redShift;
        uniform float u_blueShift;
        uniform float u_scale;
        in vec2 v_uv;
        out vec4 fragColor;

        #define PI 3.14159265359

        vec3 permute(vec3 x) { return mod(((x * 34.0) + 1.0) * x, 289.0); }

        float snoise(vec2 v) {
          const vec4 C = vec4(0.211324865405187, 0.366025403784439, -0.577350269189626, 0.024390243902439);
          vec2 i = floor(v + dot(v, C.yy));
          vec2 x0 = v - i + dot(i, C.xx);
          vec2 i1 = (x0.x > x0.y) ? vec2(1.0, 0.0) : vec2(0.0, 1.0);
          vec4 x12 = x0.xyxy + C.xxzz;
          x12.xy -= i1;
          i = mod(i, 289.0);
          vec3 p = permute(permute(i.y + vec3(0.0, i1.y, 1.0)) + i.x + vec3(0.0, i1.x, 1.0));
          vec3 m = max(0.5 - vec3(dot(x0, x0), dot(x12.xy, x12.xy), dot(x12.zw, x12.zw)), 0.0);
          m = m * m;
          m = m * m;
          vec3 x = 2.0 * fract(p * C.www) - 1.0;
          vec3 h = abs(x) - 0.5;
          vec3 ox = floor(x + 0.5);
          vec3 a0 = x - ox;
          m *= 1.79284291400159 - 0.85373472095314 * (a0 * a0 + h * h);
          vec3 g;
          g.x = a0.x * x0.x + h.x * x0.y;
          g.yz = a0.yz * x12.xz + h.yz * x12.yw;
          return 130.0 * dot(m, g);
        }

        float sdRoundedBox(vec2 p, vec2 b, float r) {
          vec2 q = abs(p) - b + r;
          return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
        }

        float getColorChanges(float c1, float c2, float stripe_p, vec3 w, float blur, float bump, float tint) {
          float ch = mix(c2, c1, smoothstep(0.0, 2.0 * blur, stripe_p));
          float border = w[0];
          ch = mix(ch, c2, smoothstep(border, border + 2.0 * blur, stripe_p));
          bump = smoothstep(0.2, 0.8, bump);
          border = w[0] + 0.4 * (1.0 - bump) * w[1];
          ch = mix(ch, c1, smoothstep(border, border + 2.0 * blur, stripe_p));
          border = w[0] + 0.5 * (1.0 - bump) * w[1];
          ch = mix(ch, c2, smoothstep(border, border + 2.0 * blur, stripe_p));
          border = w[0] + w[1];
          ch = mix(ch, c1, smoothstep(border, border + 2.0 * blur, stripe_p));
          float gradient_t = (stripe_p - w[0] - w[1]) / w[2];
          float gradient = mix(c1, c2, smoothstep(0.0, 1.0, gradient_t));
          ch = mix(ch, gradient, smoothstep(border, border + 0.5 * blur, stripe_p));
          ch = mix(ch, 1.0 - min(1.0, (1.0 - ch) / max(tint, 0.0001)), 1.0);
          return ch;
        }

        vec3 liquidMetalColor(vec2 uv) {
          float t = 0.1 * (u_time + 2.8);
          vec2 shaderUV = (uv - 0.5) * u_scale + 0.5;

          vec2 rotatedUV = shaderUV - 0.5;
          float angle = u_angle;
          rotatedUV = vec2(
            rotatedUV.x * cos(angle) - rotatedUV.y * sin(angle),
            rotatedUV.x * sin(angle) + rotatedUV.y * cos(angle)
          ) + 0.5;

          float diagBLtoTR = rotatedUV.x - rotatedUV.y;
          float diagTLtoBR = rotatedUV.x + rotatedUV.y;

          vec3 color1 = vec3(0.98, 0.98, 1.0);
          vec3 color2 = vec3(0.1, 0.1, 0.1 + 0.1 * smoothstep(0.7, 1.3, diagTLtoBR));

          vec2 grad_uv = shaderUV - 0.5;
          float dist = length(grad_uv + vec2(0.0, 0.2 * diagBLtoTR));
          float bump = pow(u_contour * dist, 1.2);
          bump = 1.0 - bump;
          bump *= pow(shaderUV.y, 0.3);

          float cycleWidth = u_repetition;
          float thin_strip_1_ratio = 0.12 / cycleWidth * (1.0 - 0.4 * bump);
          float thin_strip_2_ratio = 0.07 / cycleWidth * (1.0 + 0.4 * bump);
          float wide_strip_ratio = (1.0 - thin_strip_1_ratio - thin_strip_2_ratio);

          float noise = snoise(shaderUV - t);

          float direction = grad_uv.x + diagBLtoTR;
          direction -= u_distortion * noise * diagBLtoTR;
          direction *= cycleWidth;
          direction -= t;

          float dispersionRed = (1.0 - bump) * u_redShift;
          float dispersionBlue = (1.0 - bump) * u_blueShift;

          float blur = u_softness;
          vec3 w = vec3(cycleWidth * thin_strip_1_ratio, cycleWidth * thin_strip_2_ratio, wide_strip_ratio);

          float r = getColorChanges(color1.r, color2.r, fract(direction + dispersionRed), w, blur, bump, 1.0);
          float g = getColorChanges(color1.g, color2.g, fract(direction), w, blur, bump, 1.0);
          float b = getColorChanges(color1.b, color2.b, fract(direction - dispersionBlue), w, blur, bump, 1.0);

          return vec3(r, g, b);
        }

        vec3 experimentalColor(vec2 pixelCoord) {
          vec2 uv = pixelCoord / u_resolution;
          vec2 p = uv - 0.5;
          p.x *= u_resolution.x / max(u_resolution.y, 1.0);

          float t = u_time * 0.06;
          float flowA = snoise(p * 2.4 + vec2(-t * 0.55, t * 0.22));
          float flowB = snoise(p * 4.8 + vec2(t * 0.35, -t * 0.46));
          float flowC = snoise((p + vec2(flowA * 0.12, flowB * 0.12)) * 7.2 - vec2(t * 0.18, t * 0.11));

          float bandPrimary = 0.5 + 0.5 * sin((p.x * 7.0 - p.y * 4.6 + flowA * 1.6 + flowC * 0.55) * PI);
          float bandSecondary = 0.5 + 0.5 * sin((p.x * -3.8 - p.y * 6.2 + flowB * 1.4 - t * 0.9) * PI);
          float sheen = pow(bandPrimary, 2.8) * 0.65 + pow(bandSecondary, 4.0) * 0.35;
          float pocket = clamp(0.5 + 0.5 * (flowA * 0.72 + flowB * 0.28), 0.0, 1.0);
          float pocketRidge = smoothstep(0.12, 0.88, 0.5 + 0.5 * flowC);
          float vignette = 1.0 - smoothstep(0.22, 0.96, length(p * vec2(0.95, 1.18)));
          float hotspot = smoothstep(0.78, 0.08, length(p - vec2(0.18, -0.12)) + 0.12 * flowC);

          vec3 dark = vec3(0.07, 0.075, 0.085);
          vec3 steel = vec3(0.40, 0.44, 0.50);
          vec3 silver = vec3(0.90, 0.93, 0.97);
          vec3 pearl = vec3(0.995, 0.995, 1.0);
          vec3 coolTint = vec3(0.15, 0.24, 0.39);
          vec3 warmTint = vec3(0.34, 0.20, 0.09);

          vec3 color = mix(dark, steel, pocket);
          color = mix(color, silver, clamp(sheen * 0.82 + pocketRidge * 0.16, 0.0, 1.0));
          color += coolTint * 0.11 * smoothstep(0.18, 0.95, 0.5 + 0.5 * flowB);
          color += warmTint * 0.08 * smoothstep(0.34, 1.0, 0.5 + 0.5 * sin((p.x + p.y) * 8.2 - t));
          color += pearl * hotspot * 0.14;
          color += pearl * pow(max(vignette, 0.0), 2.0) * 0.05;
          return clamp(color, 0.0, 1.0);
        }

        void main() {
          vec2 pixelCoord = (v_uv * 0.5 + 0.5) * u_resolution;
          pixelCoord.y = u_resolution.y - pixelCoord.y;

          vec2 center = u_resolution * 0.5;
          vec2 halfSize = u_resolution * 0.5;
          float surfaceDist = sdRoundedBox(pixelCoord - center, halfSize, u_radius);
          if (surfaceDist > 0.0) {
            discard;
          }
          vec2 uv = v_uv * 0.5 + 0.5;
          uv.y = 1.0 - uv.y;
          vec3 color = u_variant > 0.5 ? experimentalColor(pixelCoord) : liquidMetalColor(uv);

          float alpha = smoothstep(1.2, -1.2, surfaceDist);
          fragColor = vec4(color, alpha);
        }
      `;

      const vs = gl.createShader(gl.VERTEX_SHADER);
      gl.shaderSource(vs, vertexShader);
      gl.compileShader(vs);
      if (!gl.getShaderParameter(vs, gl.COMPILE_STATUS)) {
        return;
      }

      const fs = gl.createShader(gl.FRAGMENT_SHADER);
      gl.shaderSource(fs, fragmentShader);
      gl.compileShader(fs);
      if (!gl.getShaderParameter(fs, gl.COMPILE_STATUS)) {
        return;
      }

      this.program = gl.createProgram();
      gl.attachShader(this.program, vs);
      gl.attachShader(this.program, fs);
      gl.linkProgram(this.program);
      if (!gl.getProgramParameter(this.program, gl.LINK_STATUS)) {
        return;
      }

      const posLoc = gl.getAttribLocation(this.program, 'a_position');
      const posBuf = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, posBuf);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]), gl.STATIC_DRAW);
      gl.enableVertexAttribArray(posLoc);
      gl.vertexAttribPointer(posLoc, 2, gl.FLOAT, false, 0, 0);

      this.uTime = gl.getUniformLocation(this.program, 'u_time');
      this.uRes = gl.getUniformLocation(this.program, 'u_resolution');
      this.uRadius = gl.getUniformLocation(this.program, 'u_radius');
      this.uVariant = gl.getUniformLocation(this.program, 'u_variant');
      this.uRepetition = gl.getUniformLocation(this.program, 'u_repetition');
      this.uContour = gl.getUniformLocation(this.program, 'u_contour');
      this.uDistortion = gl.getUniformLocation(this.program, 'u_distortion');
      this.uSoftness = gl.getUniformLocation(this.program, 'u_softness');
      this.uAngle = gl.getUniformLocation(this.program, 'u_angle');
      this.uRedShift = gl.getUniformLocation(this.program, 'u_redShift');
      this.uBlueShift = gl.getUniformLocation(this.program, 'u_blueShift');
      this.uScale = gl.getUniformLocation(this.program, 'u_scale');
    }

    render = (time) => {
      if (this.disposed || !this.gl || !this.program) {
        return;
      }
      if (!this.canvas || !this.canvas.isConnected) {
        return;
      }

      const dt = time - this.lastTime;
      this.lastTime = time;
      this.currentFrame += dt * this.speed;

      const gl = this.gl;
      if (typeof gl.isContextLost === 'function' && gl.isContextLost()) {
        this.rafId = requestAnimationFrame(this.render);
        return;
      }

      if (this.canvas.width <= 1 || this.canvas.height <= 1) {
        this.queueResize();
        this.rafId = requestAnimationFrame(this.render);
        return;
      }

      gl.enable(gl.BLEND);
      gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);

      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT);
      gl.viewport(0, 0, this.canvas.width, this.canvas.height);
      gl.useProgram(this.program);

      gl.uniform1f(this.uTime, this.currentFrame * 0.001);
      gl.uniform2f(this.uRes, this.canvas.width, this.canvas.height);
      gl.uniform1f(this.uRadius, this.borderRadius * Math.min(window.devicePixelRatio || 1, 2));
      gl.uniform1f(this.uVariant, this.variant);
      gl.uniform1f(this.uRepetition, this.params.repetition);
      gl.uniform1f(this.uContour, this.params.contour);
      gl.uniform1f(this.uDistortion, this.params.distortion);
      gl.uniform1f(this.uSoftness, this.params.softness);
      gl.uniform1f(this.uAngle, this.params.angle);
      gl.uniform1f(this.uRedShift, this.params.redShift);
      gl.uniform1f(this.uBlueShift, this.params.blueShift);
      gl.uniform1f(this.uScale, this.params.scale);

      gl.drawArrays(gl.TRIANGLES, 0, 6);

      this.rafId = requestAnimationFrame(this.render);
    };

    start() {
      this.lastTime = performance.now();
      this.rafId = requestAnimationFrame(this.render);
    }

    dispose() {
      this.disposed = true;
      if (this.rafId) {
        cancelAnimationFrame(this.rafId);
      }
      if (this.resizeRafId) {
        cancelAnimationFrame(this.resizeRafId);
      }
      if (this.resizeObserver) {
        this.resizeObserver.disconnect();
      }
      window.removeEventListener('resize', this.queueResize);
      if (this.gl && this.program) {
        this.gl.deleteProgram(this.program);
      }
      if (this.canvas && this.canvas.parentNode) {
        this.canvas.parentNode.removeChild(this.canvas);
      }
    }

    updateOptions(options = {}) {
      this.applyOptions(options);
      return this;
    }
  }

  return {
    defaultControls: Object.freeze({ ...DEFAULT_SHADER_CONTROLS }),
    defaultParams: Object.freeze({ ...DEFAULT_SHADER_PARAMS }),
    applyTo: function(element, options) {
      if (!element) {
        return null;
      }
      return new SurfaceShader(element, options);
    }
  };
})();
