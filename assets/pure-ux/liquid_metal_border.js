// Liquid Metal Border Component - WebGL border shader with rounded rectangle support
window.LiquidMetalBorder = (function() {
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

  class BorderShader {
    constructor(element, options = {}) {
      this.element = element;
      this.borderWidth = 2;
      this.animated = true;
      this.speed = DEFAULT_SHADER_SPEED;
      this.params = { ...DEFAULT_SHADER_PARAMS };
      this.disposed = false;
      this.canvas = null;
      this.gl = null;
      this.rafId = null;
      this.resizeRafId = null;
      this.resizeObserver = null;
      this.currentFrame = 0;
      this.lastTime = 0;
      this.applyOptions(options);
      
      this.initCanvas();
      this.initShader();
      this.start();
    }

    applyOptions(options = {}) {
      this.borderWidth = Math.max(0.0, readOptionNumber(options, ['borderWidth'], this.borderWidth));
      this.animated = options.animated !== false;
      this.speed = Math.max(0.0, readOptionNumber(options, ['speed'], this.speed));
      this.params = normalizeShaderParams(options, this.params);
    }

    updateShapeMetrics(rect) {
      const computedStyle = window.getComputedStyle(this.element);
      const borderRadiusStr = computedStyle.borderRadius || '0px';
      const borderRadiusValues = borderRadiusStr.split(' ').map(v => parseFloat(v) || 0);
      const rawBorderRadius = borderRadiusValues[0];
      const minDimension = Math.min(rect.width, rect.height);
      this.borderRadius = Math.min(rawBorderRadius, minDimension * 0.5);
      const isNearSquare = Math.abs(rect.width - rect.height) <= 1.5;
      this.isCircular = isNearSquare && this.borderRadius >= (minDimension / 2) - 1;
    }

    syncCanvasSize() {
      if (!this.canvas) return;

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
      // Make parent relative
      const position = window.getComputedStyle(this.element).position;
      if (position === 'static') {
        this.element.style.position = 'relative';
      }
      
      // Get dimensions and border-radius
      const rect = this.element.getBoundingClientRect();
      this.updateShapeMetrics(rect);
      
      console.log('Border shape detection:', {
        width: rect.width,
        height: rect.height,
        borderRadius: this.borderRadius,
        isCircular: this.isCircular
      });
      
      // Create canvas
      this.canvas = document.createElement('canvas');
      this.canvas.className = 'liquid-metal-border-canvas';
      this.canvas.style.cssText = `
        position: absolute;
        top: 0;
        left: 0;
        width: 100%;
        height: 100%;
        pointer-events: none;
        z-index: 1000;
      `;
      
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      this.canvas.width = Math.max(1, Math.round(rect.width * dpr));
      this.canvas.height = Math.max(1, Math.round(rect.height * dpr));
      
      this.element.appendChild(this.canvas);
      
      // Get WebGL context
      this.gl = this.canvas.getContext('webgl2', {
        alpha: true,
        premultipliedAlpha: false,
        antialias: true
      });
      
      if (!this.gl) {
        console.error('WebGL2 not supported');
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
      
      console.log('Border canvas created:', this.canvas.width, 'x', this.canvas.height, 'radius:', this.borderRadius);
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
      
      // Fragment shader with rounded rectangle and circular border support
      const fragmentShader = `#version 300 es
        precision mediump float;
        uniform vec2 u_resolution;
        uniform float u_time;
        uniform float u_borderWidth;
        uniform float u_borderRadius;
        uniform float u_isCircular;
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
        
        vec3 permute(vec3 x) { return mod(((x*34.0)+1.0)*x, 289.0); }
        
        float snoise(vec2 v) {
          const vec4 C = vec4(0.211324865405187, 0.366025403784439, -0.577350269189626, 0.024390243902439);
          vec2 i = floor(v + dot(v, C.yy));
          vec2 x0 = v - i + dot(i, C.xx);
          vec2 i1 = (x0.x > x0.y) ? vec2(1.0, 0.0) : vec2(0.0, 1.0);
          vec4 x12 = x0.xyxy + C.xxzz;
          x12.xy -= i1;
          i = mod(i, 289.0);
          vec3 p = permute(permute(i.y + vec3(0.0, i1.y, 1.0)) + i.x + vec3(0.0, i1.x, 1.0));
          vec3 m = max(0.5 - vec3(dot(x0,x0), dot(x12.xy,x12.xy), dot(x12.zw,x12.zw)), 0.0);
          m = m*m;
          m = m*m;
          vec3 x = 2.0 * fract(p * C.www) - 1.0;
          vec3 h = abs(x) - 0.5;
          vec3 ox = floor(x + 0.5);
          vec3 a0 = x - ox;
          m *= 1.79284291400159 - 0.85373472095314 * (a0*a0 + h*h);
          vec3 g;
          g.x = a0.x * x0.x + h.x * x0.y;
          g.yz = a0.yz * x12.xz + h.yz * x12.yw;
          return 130.0 * dot(m, g);
        }
        
        // Signed distance function for circle
        float sdCircle(vec2 p, float r) {
          return length(p) - r;
        }
        
        // Signed distance function for rounded rectangle
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
        
        void main() {
          // Convert from clip space to pixel coordinates
          vec2 pixelCoord = (v_uv * 0.5 + 0.5) * u_resolution;
          pixelCoord.y = u_resolution.y - pixelCoord.y; // Flip Y
          
          // Calculate center and size
          vec2 center = u_resolution * 0.5;
          vec2 halfSize = u_resolution * 0.5;
          
          float outerDist, innerDist;
          
          if (u_isCircular > 0.5) {
            // Circular border
            float radius = min(halfSize.x, halfSize.y);
            outerDist = sdCircle(pixelCoord - center, radius);
            innerDist = sdCircle(pixelCoord - center, radius - u_borderWidth);
          } else {
            // Rounded rectangle border
            outerDist = sdRoundedBox(pixelCoord - center, halfSize, u_borderRadius);
            innerDist = sdRoundedBox(pixelCoord - center, halfSize - u_borderWidth, max(0.0, u_borderRadius - u_borderWidth));
          }
          
          // Only render in border region
          if (outerDist > 0.0 || innerDist < 0.0) {
            discard;
          }
          
          // UV coordinates for shader effect - slowed down animation
          float t = 0.1 * (u_time + 2.8);
          vec2 uv = v_uv * 0.5 + 0.5;
          uv.y = 1.0 - uv.y;
          vec2 shaderUV = (uv - 0.5) * u_scale + 0.5;
          
          // Liquid metal shader effect
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
          
          vec3 color = vec3(r, g, b);
          
          // Anti-aliased border edges
          float edgeSmoothness = 1.0;
          float alpha = smoothstep(edgeSmoothness, -edgeSmoothness, outerDist) * 
                       smoothstep(-edgeSmoothness, edgeSmoothness, innerDist);
          
          fragColor = vec4(color, alpha);
        }
      `;
      
      const vs = gl.createShader(gl.VERTEX_SHADER);
      gl.shaderSource(vs, vertexShader);
      gl.compileShader(vs);
      if (!gl.getShaderParameter(vs, gl.COMPILE_STATUS)) {
        console.error('Vertex shader error:', gl.getShaderInfoLog(vs));
      }
      
      const fs = gl.createShader(gl.FRAGMENT_SHADER);
      gl.shaderSource(fs, fragmentShader);
      gl.compileShader(fs);
      if (!gl.getShaderParameter(fs, gl.COMPILE_STATUS)) {
        console.error('Fragment shader error:', gl.getShaderInfoLog(fs));
      }
      
      this.program = gl.createProgram();
      gl.attachShader(this.program, vs);
      gl.attachShader(this.program, fs);
      gl.linkProgram(this.program);
      
      if (!gl.getProgramParameter(this.program, gl.LINK_STATUS)) {
        console.error('Program link error:', gl.getProgramInfoLog(this.program));
      }
      
      const posLoc = gl.getAttribLocation(this.program, 'a_position');
      const posBuf = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, posBuf);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1,-1, 1,-1, -1,1, -1,1, 1,-1, 1,1]), gl.STATIC_DRAW);
      gl.enableVertexAttribArray(posLoc);
      gl.vertexAttribPointer(posLoc, 2, gl.FLOAT, false, 0, 0);
      
      this.uTime = gl.getUniformLocation(this.program, 'u_time');
      this.uRes = gl.getUniformLocation(this.program, 'u_resolution');
      this.uBorderWidth = gl.getUniformLocation(this.program, 'u_borderWidth');
      this.uBorderRadius = gl.getUniformLocation(this.program, 'u_borderRadius');
      this.uIsCircular = gl.getUniformLocation(this.program, 'u_isCircular');
      this.uRepetition = gl.getUniformLocation(this.program, 'u_repetition');
      this.uContour = gl.getUniformLocation(this.program, 'u_contour');
      this.uDistortion = gl.getUniformLocation(this.program, 'u_distortion');
      this.uSoftness = gl.getUniformLocation(this.program, 'u_softness');
      this.uAngle = gl.getUniformLocation(this.program, 'u_angle');
      this.uRedShift = gl.getUniformLocation(this.program, 'u_redShift');
      this.uBlueShift = gl.getUniformLocation(this.program, 'u_blueShift');
      this.uScale = gl.getUniformLocation(this.program, 'u_scale');
      
      console.log('Border shader initialized successfully');
    }
    
    render = (time) => {
      if (this.disposed || !this.gl) return;
      if (!this.canvas || !this.canvas.isConnected) return;
      
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
      
      // Enable blending
      gl.enable(gl.BLEND);
      gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
      
      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT);
      gl.viewport(0, 0, this.canvas.width, this.canvas.height);
      gl.useProgram(this.program);
      
      gl.uniform1f(this.uTime, this.currentFrame * 0.001);
      gl.uniform2f(this.uRes, this.canvas.width, this.canvas.height);
      gl.uniform1f(this.uBorderWidth, this.borderWidth * (Math.min(window.devicePixelRatio || 1, 2)));
      gl.uniform1f(this.uBorderRadius, this.borderRadius * (Math.min(window.devicePixelRatio || 1, 2)));
      gl.uniform1f(this.uIsCircular, this.isCircular ? 1.0 : 0.0);
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
      if (this.rafId) cancelAnimationFrame(this.rafId);
      if (this.resizeRafId) cancelAnimationFrame(this.resizeRafId);
      if (this.resizeObserver) this.resizeObserver.disconnect();
      window.removeEventListener('resize', this.queueResize);
      if (this.gl && this.program) this.gl.deleteProgram(this.program);
      if (this.canvas && this.canvas.parentNode) {
        this.canvas.parentNode.removeChild(this.canvas);
      }
    }

    updateOptions(options = {}) {
      this.applyOptions(options);
      return this;
    }
  }
  
  // Public API
  return {
    defaultControls: Object.freeze({ ...DEFAULT_SHADER_CONTROLS }),
    defaultParams: Object.freeze({ ...DEFAULT_SHADER_PARAMS }),
    create: function(elementId, options) {
      const element = document.getElementById(elementId);
      if (!element) {
        console.error('LiquidMetalBorder: Element not found:', elementId);
        return null;
      }
      return new BorderShader(element, options);
    },
    
    applyTo: function(element, options) {
      if (!element) {
        console.error('LiquidMetalBorder: No element provided');
        return null;
      }
      return new BorderShader(element, options);
    }
  };
})();

window.LiquidMetalBorderGroup = (function() {
  'use strict';

  const MAX_GROUP_BOXES = 8;
  const FALLBACK_SHADER_CONTROLS = Object.freeze({
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
  const BASE_CONTROLS = (window.LiquidMetalBorder && window.LiquidMetalBorder.defaultControls)
    || FALLBACK_SHADER_CONTROLS;

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

  const DEFAULT_GROUP_SPEED = pctToUnit(BASE_CONTROLS.speedPct);
  const DEFAULT_GROUP_PARAMS = Object.freeze({
    ...createShaderParamsFromControls(BASE_CONTROLS)
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

  function normalizeShaderParams(options, base = DEFAULT_GROUP_PARAMS) {
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

  class BorderGroupShader {
    constructor(element, options = {}) {
      this.element = element;
      this.selector = '.liquid-metal-target';
      this.borderWidth = 2;
      this.animated = true;
      this.speed = DEFAULT_GROUP_SPEED;
      this.params = { ...DEFAULT_GROUP_PARAMS };
      this.loopDurationMs = 4200;
      this.ambientStrength = 0.18;
      this.sweepStrength = 0.92;
      this.sweepWidth = 0.16;
      this.disposed = false;
      this.canvas = null;
      this.gl = null;
      this.rafId = null;
      this.resizeRafId = null;
      this.resizeObserver = null;
      this.currentFrame = 0;
      this.elapsedMs = 0;
      this.lastTime = 0;
      this.targetElements = [];
      this.boxData = new Float32Array(MAX_GROUP_BOXES * 4);
      this.boxRadiusData = new Float32Array(MAX_GROUP_BOXES);
      this.boxCount = 0;
      this.applyOptions(options);

      this.initCanvas();
      this.initShader();
      this.start();
    }

    applyOptions(options = {}) {
      if (typeof options.selector === 'string' && options.selector.trim()) {
        this.selector = options.selector.trim();
      }
      this.borderWidth = Math.max(0.0, readOptionNumber(options, ['borderWidth'], this.borderWidth));
      this.animated = options.animated !== false;
      this.speed = Math.max(0.0, readOptionNumber(options, ['speed'], this.speed));
      this.loopDurationMs = Math.max(1200, readOptionNumber(options, ['loopDurationMs', 'loopMs'], this.loopDurationMs));
      this.ambientStrength = Math.max(0.0, Math.min(1.0, readOptionNumber(options, ['ambientStrength'], this.ambientStrength)));
      this.sweepStrength = Math.max(0.0, Math.min(2.0, readOptionNumber(options, ['sweepStrength'], this.sweepStrength)));
      this.sweepWidth = Math.max(0.04, Math.min(0.5, readOptionNumber(options, ['sweepWidth'], this.sweepWidth)));
      this.params = normalizeShaderParams(options, this.params);
    }

    setReadyState(isReady) {
      if (!this.element) return;
      this.element.classList.toggle('hero-ripple-ready', isReady);
      this.targetElements.forEach((target) => {
        target.classList.toggle('liquid-metal-ready', isReady);
      });
    }

    refreshTargets() {
      if (!this.element) {
        this.targetElements = [];
        return;
      }
      this.targetElements = Array.from(this.element.querySelectorAll(this.selector))
        .filter((target) => target && target.isConnected);
    }

    syncCanvasSize() {
      if (!this.canvas) return;

      const rect = this.element.getBoundingClientRect();
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      const nextWidth = Math.max(1, Math.round(rect.width * dpr));
      const nextHeight = Math.max(1, Math.round(rect.height * dpr));

      if (this.canvas.width !== nextWidth || this.canvas.height !== nextHeight) {
        this.canvas.width = nextWidth;
        this.canvas.height = nextHeight;
      }

      this.refreshTargets();
      this.boxData.fill(0);
      this.boxRadiusData.fill(0);
      this.boxCount = 0;

      this.targetElements.slice(0, MAX_GROUP_BOXES).forEach((target, idx) => {
        const targetRect = target.getBoundingClientRect();
        const computedStyle = window.getComputedStyle(target);
        const rawRadius = parseFloat(computedStyle.borderTopLeftRadius || '0') || 0;
        const minDimension = Math.min(targetRect.width, targetRect.height);
        const radius = Math.min(rawRadius, minDimension * 0.5);
        const baseOffset = idx * 4;

        this.boxData[baseOffset] = (targetRect.left - rect.left + targetRect.width * 0.5) * dpr;
        this.boxData[baseOffset + 1] = (targetRect.top - rect.top + targetRect.height * 0.5) * dpr;
        this.boxData[baseOffset + 2] = targetRect.width * 0.5 * dpr;
        this.boxData[baseOffset + 3] = targetRect.height * 0.5 * dpr;
        this.boxRadiusData[idx] = radius * dpr;
        this.boxCount += 1;
      });

      this.setReadyState(this.boxCount > 0 && nextWidth > 4 && nextHeight > 4);

      if (this.resizeObserver) {
        this.resizeObserver.disconnect();
        this.resizeObserver.observe(this.element);
        this.targetElements.forEach((target) => {
          this.resizeObserver.observe(target);
        });
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

      this.canvas = document.createElement('canvas');
      this.canvas.className = 'liquid-metal-border-group-canvas';
      this.canvas.style.cssText = `
        position: absolute;
        inset: 0;
        width: 100%;
        height: 100%;
        pointer-events: none;
        z-index: 1;
      `;
      this.element.insertBefore(this.canvas, this.element.firstChild);

      this.gl = this.canvas.getContext('webgl2', {
        alpha: true,
        premultipliedAlpha: false,
        antialias: true
      });

      if (!this.gl) {
        console.error('LiquidMetalBorderGroup: WebGL2 not supported');
        return;
      }

      window.addEventListener('resize', this.queueResize, { passive: true });
      if (typeof ResizeObserver === 'function') {
        this.resizeObserver = new ResizeObserver(() => {
          this.queueResize();
        });
      }

      this.syncCanvasSize();
    }

    initShader() {
      const gl = this.gl;
      if (!gl) return;

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
        uniform float u_borderWidth;
        uniform int u_boxCount;
        uniform vec4 u_boxes[${MAX_GROUP_BOXES}];
        uniform float u_boxRadii[${MAX_GROUP_BOXES}];
        uniform float u_repetition;
        uniform float u_contour;
        uniform float u_distortion;
        uniform float u_softness;
        uniform float u_angle;
        uniform float u_redShift;
        uniform float u_blueShift;
        uniform float u_scale;
        uniform float u_loopProgress;
        uniform float u_ambientStrength;
        uniform float u_sweepStrength;
        uniform float u_sweepWidth;
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

        float getColorChanges(float c1, float c2, float stripeP, vec3 w, float blur, float bump, float tint) {
          float ch = mix(c2, c1, smoothstep(0.0, 2.0 * blur, stripeP));
          float border = w[0];
          ch = mix(ch, c2, smoothstep(border, border + 2.0 * blur, stripeP));
          bump = smoothstep(0.2, 0.8, bump);
          border = w[0] + 0.4 * (1.0 - bump) * w[1];
          ch = mix(ch, c1, smoothstep(border, border + 2.0 * blur, stripeP));
          border = w[0] + 0.5 * (1.0 - bump) * w[1];
          ch = mix(ch, c2, smoothstep(border, border + 2.0 * blur, stripeP));
          border = w[0] + w[1];
          ch = mix(ch, c1, smoothstep(border, border + 2.0 * blur, stripeP));
          float gradientT = (stripeP - w[0] - w[1]) / w[2];
          float gradient = mix(c1, c2, smoothstep(0.0, 1.0, gradientT));
          ch = mix(ch, gradient, smoothstep(border, border + 0.5 * blur, stripeP));
          ch = mix(ch, 1.0 - min(1.0, (1.0 - ch) / max(tint, 0.0001)), 1.0);
          return ch;
        }

        void main() {
          vec2 pixelCoord = (v_uv * 0.5 + 0.5) * u_resolution;
          pixelCoord.y = u_resolution.y - pixelCoord.y;
          vec2 rowUV = pixelCoord / u_resolution;

          float ringAlpha = 0.0;
          vec2 bestLocalUV = vec2(0.5);

          for (int i = 0; i < ${MAX_GROUP_BOXES}; i++) {
            if (i >= u_boxCount) {
              break;
            }

            vec4 box = u_boxes[i];
            vec2 halfSize = box.zw;
            float radius = u_boxRadii[i];
            float outerDist = sdRoundedBox(pixelCoord - box.xy, halfSize, radius);
            vec2 innerHalf = max(halfSize - vec2(u_borderWidth), vec2(0.0));
            float innerDist = sdRoundedBox(
              pixelCoord - box.xy,
              innerHalf,
              max(0.0, radius - u_borderWidth)
            );
            float alpha = smoothstep(1.0, -1.0, outerDist)
              * smoothstep(-1.0, 1.0, innerDist);

            if (alpha > ringAlpha) {
              ringAlpha = alpha;
              vec2 minCorner = box.xy - halfSize;
              vec2 size = max(halfSize * 2.0, vec2(1.0));
              bestLocalUV = clamp((pixelCoord - minCorner) / size, 0.0, 1.0);
            }
          }

          if (ringAlpha <= 0.001) {
            discard;
          }

          float t = 0.1 * (u_time + 2.8);
          vec2 shaderUV = vec2(rowUV.x, bestLocalUV.y);
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

          vec2 gradUV = vec2(rowUV.x - 0.5, bestLocalUV.y - 0.5);
          float dist = length(gradUV + vec2(0.0, 0.2 * diagBLtoTR));
          float bump = pow(u_contour * dist, 1.2);
          bump = 1.0 - bump;
          bump *= pow(bestLocalUV.y, 0.3);

          float cycleWidth = u_repetition;
          float thinStrip1Ratio = 0.12 / cycleWidth * (1.0 - 0.4 * bump);
          float thinStrip2Ratio = 0.07 / cycleWidth * (1.0 + 0.4 * bump);
          float wideStripRatio = (1.0 - thinStrip1Ratio - thinStrip2Ratio);

          float noise = snoise(shaderUV - t);
          float direction = gradUV.x + diagBLtoTR;
          direction -= u_distortion * noise * diagBLtoTR;
          direction *= cycleWidth;
          direction -= t;

          float dispersionRed = (1.0 - bump) * u_redShift;
          float dispersionBlue = (1.0 - bump) * u_blueShift;
          float blur = u_softness;
          vec3 w = vec3(cycleWidth * thinStrip1Ratio, cycleWidth * thinStrip2Ratio, wideStripRatio);

          float r = getColorChanges(color1.r, color2.r, fract(direction + dispersionRed), w, blur, bump, 1.0);
          float g = getColorChanges(color1.g, color2.g, fract(direction), w, blur, bump, 1.0);
          float b = getColorChanges(color1.b, color2.b, fract(direction - dispersionBlue), w, blur, bump, 1.0);

          vec3 liquidColor = vec3(r, g, b);
          float sweepStart = 0.06;
          float sweepEnd = 0.64;
          float sweepTravel = clamp((u_loopProgress - sweepStart) / (sweepEnd - sweepStart), 0.0, 1.0);
          float sweepActive = step(sweepStart, u_loopProgress) * (1.0 - step(sweepEnd, u_loopProgress));
          float sweepHead = mix(-u_sweepWidth, 1.0 + u_sweepWidth, sweepTravel);
          float sweepCurve = 0.024 * sin((rowUV.x * 0.7 + bestLocalUV.y * 1.15 + sweepTravel * 1.1) * PI * 2.0);
          float sweepCoord = rowUV.x * 0.74 + bestLocalUV.y * 0.26 + sweepCurve;
          float sweep = sweepActive * exp(-pow((sweepCoord - sweepHead) / max(u_sweepWidth, 0.0001), 2.0));
          float ambientNoise = 0.5 + 0.5 * snoise(vec2(rowUV.x * 5.2 - u_time * 0.18, bestLocalUV.y * 8.0 + u_time * 0.12));
          float ambient = u_ambientStrength * (0.55 + 0.45 * ambientNoise);
          float energy = clamp(ambient + sweep * u_sweepStrength, 0.0, 1.0);

          vec3 neutral = mix(vec3(0.64, 0.66, 0.7), vec3(0.9, 0.91, 0.94), 0.35 + bump * 0.25);
          vec3 color = mix(neutral, liquidColor, 0.45 + 0.55 * energy);
          color += vec3(1.0) * (0.06 * ambient + 0.18 * sweep);

          float alpha = ringAlpha * clamp(0.22 + 0.88 * energy, 0.0, 0.98);
          fragColor = vec4(color, alpha);
        }
      `;

      const vs = gl.createShader(gl.VERTEX_SHADER);
      gl.shaderSource(vs, vertexShader);
      gl.compileShader(vs);
      if (!gl.getShaderParameter(vs, gl.COMPILE_STATUS)) {
        console.error('LiquidMetalBorderGroup: Vertex shader error:', gl.getShaderInfoLog(vs));
      }

      const fs = gl.createShader(gl.FRAGMENT_SHADER);
      gl.shaderSource(fs, fragmentShader);
      gl.compileShader(fs);
      if (!gl.getShaderParameter(fs, gl.COMPILE_STATUS)) {
        console.error('LiquidMetalBorderGroup: Fragment shader error:', gl.getShaderInfoLog(fs));
      }

      this.program = gl.createProgram();
      gl.attachShader(this.program, vs);
      gl.attachShader(this.program, fs);
      gl.linkProgram(this.program);

      if (!gl.getProgramParameter(this.program, gl.LINK_STATUS)) {
        console.error('LiquidMetalBorderGroup: Program link error:', gl.getProgramInfoLog(this.program));
      }

      const posLoc = gl.getAttribLocation(this.program, 'a_position');
      const posBuf = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, posBuf);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]), gl.STATIC_DRAW);
      gl.enableVertexAttribArray(posLoc);
      gl.vertexAttribPointer(posLoc, 2, gl.FLOAT, false, 0, 0);

      this.uTime = gl.getUniformLocation(this.program, 'u_time');
      this.uRes = gl.getUniformLocation(this.program, 'u_resolution');
      this.uBorderWidth = gl.getUniformLocation(this.program, 'u_borderWidth');
      this.uBoxCount = gl.getUniformLocation(this.program, 'u_boxCount');
      this.uBoxes = gl.getUniformLocation(this.program, 'u_boxes[0]');
      this.uBoxRadii = gl.getUniformLocation(this.program, 'u_boxRadii[0]');
      this.uRepetition = gl.getUniformLocation(this.program, 'u_repetition');
      this.uContour = gl.getUniformLocation(this.program, 'u_contour');
      this.uDistortion = gl.getUniformLocation(this.program, 'u_distortion');
      this.uSoftness = gl.getUniformLocation(this.program, 'u_softness');
      this.uAngle = gl.getUniformLocation(this.program, 'u_angle');
      this.uRedShift = gl.getUniformLocation(this.program, 'u_redShift');
      this.uBlueShift = gl.getUniformLocation(this.program, 'u_blueShift');
      this.uScale = gl.getUniformLocation(this.program, 'u_scale');
      this.uLoopProgress = gl.getUniformLocation(this.program, 'u_loopProgress');
      this.uAmbientStrength = gl.getUniformLocation(this.program, 'u_ambientStrength');
      this.uSweepStrength = gl.getUniformLocation(this.program, 'u_sweepStrength');
      this.uSweepWidth = gl.getUniformLocation(this.program, 'u_sweepWidth');
    }

    render = (time) => {
      if (this.disposed || !this.gl) return;
      if (!this.canvas || !this.canvas.isConnected) return;

      const dt = time - this.lastTime;
      this.lastTime = time;
      this.elapsedMs += dt;
      if (this.animated) {
        this.currentFrame += dt * this.speed;
      }

      const gl = this.gl;
      if (typeof gl.isContextLost === 'function' && gl.isContextLost()) {
        this.rafId = requestAnimationFrame(this.render);
        return;
      }

      if (this.canvas.width <= 1 || this.canvas.height <= 1 || this.boxCount <= 0) {
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
      gl.uniform1f(this.uBorderWidth, this.borderWidth * Math.min(window.devicePixelRatio || 1, 2));
      gl.uniform1i(this.uBoxCount, this.boxCount);
      gl.uniform4fv(this.uBoxes, this.boxData);
      gl.uniform1fv(this.uBoxRadii, this.boxRadiusData);
      gl.uniform1f(this.uRepetition, this.params.repetition);
      gl.uniform1f(this.uContour, this.params.contour);
      gl.uniform1f(this.uDistortion, this.params.distortion);
      gl.uniform1f(this.uSoftness, this.params.softness);
      gl.uniform1f(this.uAngle, this.params.angle);
      gl.uniform1f(this.uRedShift, this.params.redShift);
      gl.uniform1f(this.uBlueShift, this.params.blueShift);
      gl.uniform1f(this.uScale, this.params.scale);
      gl.uniform1f(this.uLoopProgress, this.loopDurationMs > 0 ? ((this.elapsedMs % this.loopDurationMs) / this.loopDurationMs) : 0.0);
      gl.uniform1f(this.uAmbientStrength, this.ambientStrength);
      gl.uniform1f(this.uSweepStrength, this.sweepStrength);
      gl.uniform1f(this.uSweepWidth, this.sweepWidth);

      gl.drawArrays(gl.TRIANGLES, 0, 6);
      this.rafId = requestAnimationFrame(this.render);
    };

    start() {
      this.lastTime = performance.now();
      this.rafId = requestAnimationFrame(this.render);
    }

    dispose() {
      this.disposed = true;
      this.setReadyState(false);
      if (this.rafId) cancelAnimationFrame(this.rafId);
      if (this.resizeRafId) cancelAnimationFrame(this.resizeRafId);
      if (this.resizeObserver) this.resizeObserver.disconnect();
      window.removeEventListener('resize', this.queueResize);
      if (this.gl && this.program) this.gl.deleteProgram(this.program);
      if (this.canvas && this.canvas.parentNode) {
        this.canvas.parentNode.removeChild(this.canvas);
      }
    }

    updateOptions(options = {}) {
      this.applyOptions(options);
      this.queueResize();
      return this;
    }
  }

  return {
    defaultControls: Object.freeze({ ...BASE_CONTROLS }),
    defaultParams: Object.freeze({ ...DEFAULT_GROUP_PARAMS }),
    applyTo: function(element, options) {
      if (!element) {
        console.error('LiquidMetalBorderGroup: No element provided');
        return null;
      }
      return new BorderGroupShader(element, options);
    }
  };
})();
