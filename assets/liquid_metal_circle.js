// Liquid Metal Circle - Filled circle with brightness control for status indication
window.LiquidMetalCircle = (function() {
  'use strict';

  class CircleShaderMount {
    constructor(container, options = {}) {
      this.container = container;
      this.size = options.size || 48;
      this.brightness = options.brightness || 1.0;
      this.canvas = null;
      this.gl = null;
      this.disposed = false;
      this.rafId = null;
      this.currentFrame = 0;
      this.lastTime = 0;
      
      this.initCanvas();
      this.initShader();
      this.start();
    }
    
    initCanvas() {
      this.canvas = document.createElement('canvas');
      this.canvas.style.cssText = 'width:100%;height:100%;display:block;border-radius:50%;';
      this.container.appendChild(this.canvas);
      
      this.gl = this.canvas.getContext('webgl2', { 
        alpha: true, 
        premultipliedAlpha: false,
        antialias: true
      });
      
      if (!this.gl) {
        console.error('WebGL2 not supported');
        return;
      }
      
      this.resize();
      
      this.resizeObserver = new ResizeObserver(() => this.resize());
      this.resizeObserver.observe(this.container);
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
      
      // Fragment shader with brightness control
      const fragmentShader = `#version 300 es
        precision mediump float;
        uniform vec2 u_resolution;
        uniform float u_time;
        uniform float u_brightness;
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
          // Convert to texture coordinates
          vec2 texCoord = v_uv * 0.5 + 0.5;
          texCoord.y = 1.0 - texCoord.y;
          
          // Create circular mask
          vec2 center = vec2(0.5, 0.5);
          float dist = length(texCoord - center);
          float radius = 0.5;
          
          // Smooth circular edge
          float circleMask = 1.0 - smoothstep(radius - 0.02, radius, dist);
          
          if (circleMask < 0.01) {
            discard;
          }
          
          // UV coordinates for shader effect
          float t = 0.1 * (u_time + 2.8);
          vec2 uv = texCoord;
          
          // Liquid metal shader effect
          vec2 rotatedUV = uv - 0.5;
          float angle = 0.0;
          rotatedUV = vec2(
            rotatedUV.x * cos(angle) - rotatedUV.y * sin(angle),
            rotatedUV.x * sin(angle) + rotatedUV.y * cos(angle)
          ) + 0.5;
          
          float diagBLtoTR = rotatedUV.x - rotatedUV.y;
          float diagTLtoBR = rotatedUV.x + rotatedUV.y;
          
          // Adjust colors based on brightness
          vec3 color1 = vec3(0.98, 0.98, 1.0) * u_brightness;
          vec3 color2 = vec3(0.1, 0.1, 0.1 + 0.1 * smoothstep(0.7, 1.3, diagTLtoBR)) * max(u_brightness * 0.5, 0.3);
          
          vec2 grad_uv = uv - 0.5;
          float gradDist = length(grad_uv + vec2(0.0, 0.2 * diagBLtoTR));
          float bump = pow(1.8 * gradDist, 1.2);
          bump = 1.0 - bump;
          bump *= pow(uv.y, 0.3);
          
          float cycleWidth = 2.0;
          float thin_strip_1_ratio = 0.12 / cycleWidth * (1.0 - 0.4 * bump);
          float thin_strip_2_ratio = 0.07 / cycleWidth * (1.0 + 0.4 * bump);
          float wide_strip_ratio = (1.0 - thin_strip_1_ratio - thin_strip_2_ratio);
          
          float noise = snoise(uv - t);
          
          float direction = grad_uv.x + diagBLtoTR;
          direction -= 2.0 * noise * diagBLtoTR;
          direction *= cycleWidth;
          direction -= t;
          
          float dispersionRed = (1.0 - bump) * (0.3 / 20.0);
          float dispersionBlue = (1.0 - bump) * 1.3 * (0.3 / 20.0);
          
          float blur = 0.1 / 15.0;
          vec3 w = vec3(cycleWidth * thin_strip_1_ratio, cycleWidth * thin_strip_2_ratio, wide_strip_ratio);
          
          float r = getColorChanges(color1.r, color2.r, fract(direction + dispersionRed), w, blur, bump, 1.0);
          float g = getColorChanges(color1.g, color2.g, fract(direction), w, blur, bump, 1.0);
          float b = getColorChanges(color1.b, color2.b, fract(direction - dispersionBlue), w, blur, bump, 1.0);
          
          vec3 color = vec3(r, g, b);
          
          // Apply overall brightness adjustment
          color = mix(color * 0.3, color, u_brightness);
          
          fragColor = vec4(color, circleMask);
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
      this.uBrightness = gl.getUniformLocation(this.program, 'u_brightness');
      
      console.log('Liquid metal circle shader initialized with brightness control');
    }
    
    resize() {
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      const width = this.container.clientWidth;
      const height = this.container.clientHeight;
      this.canvas.width = width * dpr;
      this.canvas.height = height * dpr;
      if (this.gl) {
        this.gl.viewport(0, 0, this.canvas.width, this.canvas.height);
      }
    }
    
    // Method to update brightness dynamically
    setBrightness(value) {
      this.brightness = Math.max(0.0, Math.min(1.5, value));
    }
    
    render = (time) => {
      if (this.disposed || !this.gl) return;
      
      const dt = time - this.lastTime;
      this.lastTime = time;
      this.currentFrame += dt;
      
      const gl = this.gl;
      
      gl.enable(gl.BLEND);
      gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
      
      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT);
      gl.useProgram(this.program);
      
      gl.uniform1f(this.uTime, this.currentFrame * 0.001);
      gl.uniform2f(this.uRes, this.canvas.width, this.canvas.height);
      gl.uniform1f(this.uBrightness, this.brightness);
      
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
      if (this.resizeObserver) this.resizeObserver.disconnect();
      if (this.gl && this.program) this.gl.deleteProgram(this.program);
      if (this.canvas) this.canvas.remove();
    }
  }
  
  // Public API
  return {
    create: function(containerId, options) {
      const container = document.getElementById(containerId);
      if (!container) {
        console.error('LiquidMetalCircle: Container not found:', containerId);
        return null;
      }
      return new CircleShaderMount(container, options);
    },
    
    // Convenience method to create with specific brightness for status
    createWithStatus: function(containerId, status) {
      const brightnessMap = {
        'neutral': 0.5,
        'ok': 1.0,
        'warn': 0.7
      };
      const brightness = brightnessMap[status] || 1.0;
      return this.create(containerId, { brightness: brightness });
    }
  };
})();