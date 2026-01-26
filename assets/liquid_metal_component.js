// Liquid Metal Component - Reusable shader instances
window.LiquidMetalComponent = (function() {
  'use strict';

  // Minimal ShaderMount for component-sized instances
  class MiniShaderMount {
    constructor(container, size) {
      this.container = container;
      this.canvas = document.createElement('canvas');
      this.canvas.style.cssText = 'width:100%;height:100%;display:block;';
      this.container.appendChild(this.canvas);
      
      const gl = this.canvas.getContext('webgl2', { 
        alpha: true, 
        premultipliedAlpha: false,
        antialias: true
      });
      if (!gl) throw new Error('WebGL2 not supported');
      this.gl = gl;
      
      this.size = size || 48;
      this.rafId = null;
      this.currentFrame = 0;
      this.lastTime = 0;
      this.disposed = false;
      
      this.initShader();
      this.resize();
      this.start();
      
      // Handle container resize
      this.resizeObserver = new ResizeObserver(() => this.resize());
      this.resizeObserver.observe(this.container);
    }
    
    initShader() {
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
          float t = 0.3 * (u_time + 2.8);
          vec2 uv = v_uv * 0.5 + 0.5;
          uv.y = 1.0 - uv.y;
          
          // Circle shape
          vec2 shapeUV = uv - 0.5;
          shapeUV *= 0.67;
          float edge = pow(clamp(3.0 * length(shapeUV), 0.0, 1.0), 18.0);
          float opacity = 1.0 - smoothstep(0.9 - 2.0 * fwidth(edge), 0.9, edge);
          edge = 1.2 * edge;
          
          vec2 rotatedUV = uv - 0.5;
          float angle = 0.0;
          rotatedUV = vec2(
            rotatedUV.x * cos(angle) - rotatedUV.y * sin(angle),
            rotatedUV.x * sin(angle) + rotatedUV.y * cos(angle)
          ) + 0.5;
          
          float diagBLtoTR = rotatedUV.x - rotatedUV.y;
          float diagTLtoBR = rotatedUV.x + rotatedUV.y;
          
          vec3 color1 = vec3(0.98, 0.98, 1.0);
          vec3 color2 = vec3(0.1, 0.1, 0.1 + 0.1 * smoothstep(0.7, 1.3, diagTLtoBR));
          
          vec2 grad_uv = uv - 0.5;
          float dist = length(grad_uv + vec2(0.0, 0.2 * diagBLtoTR));
          float bump = pow(1.8 * dist, 1.2);
          bump = 1.0 - bump;
          bump *= pow(uv.y, 0.3);
          
          float cycleWidth = 2.0;
          float thin_strip_1_ratio = 0.12 / cycleWidth * (1.0 - 0.4 * bump);
          float thin_strip_2_ratio = 0.07 / cycleWidth * (1.0 + 0.4 * bump);
          float wide_strip_ratio = (1.0 - thin_strip_1_ratio - thin_strip_2_ratio);
          
          float noise = snoise(uv - t);
          edge += (1.0 - edge) * 0.07 * noise;
          
          float direction = grad_uv.x + diagBLtoTR;
          direction -= 2.0 * noise * diagBLtoTR * (smoothstep(0.0, 1.0, edge) * (1.0 - smoothstep(0.0, 1.0, edge)));
          direction *= mix(1.0, 1.0 - edge, smoothstep(0.5, 1.0, 0.4));
          direction -= 1.7 * edge * smoothstep(0.5, 1.0, 0.4);
          bump *= clamp(pow(uv.y, 0.1), 0.3, 1.0);
          direction *= (0.1 + (1.1 - edge) * bump);
          direction *= (0.4 + 0.6 * (1.0 - smoothstep(0.5, 1.0, edge)));
          direction *= (0.5 + 0.5 * pow(uv.y, 2.0));
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
          
          // Keep transparency - don't mix with background
          fragColor = vec4(color, opacity);
        }
      `;
      
      const vs = this.gl.createShader(this.gl.VERTEX_SHADER);
      this.gl.shaderSource(vs, vertexShader);
      this.gl.compileShader(vs);
      
      const fs = this.gl.createShader(this.gl.FRAGMENT_SHADER);
      this.gl.shaderSource(fs, fragmentShader);
      this.gl.compileShader(fs);
      
      this.program = this.gl.createProgram();
      this.gl.attachShader(this.program, vs);
      this.gl.attachShader(this.program, fs);
      this.gl.linkProgram(this.program);
      
      const posLoc = this.gl.getAttribLocation(this.program, 'a_position');
      const posBuf = this.gl.createBuffer();
      this.gl.bindBuffer(this.gl.ARRAY_BUFFER, posBuf);
      this.gl.bufferData(this.gl.ARRAY_BUFFER, new Float32Array([-1,-1, 1,-1, -1,1, -1,1, 1,-1, 1,1]), this.gl.STATIC_DRAW);
      this.gl.enableVertexAttribArray(posLoc);
      this.gl.vertexAttribPointer(posLoc, 2, this.gl.FLOAT, false, 0, 0);
      
      this.uTime = this.gl.getUniformLocation(this.program, 'u_time');
      this.uRes = this.gl.getUniformLocation(this.program, 'u_resolution');
    }
    
    resize() {
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      const width = this.container.clientWidth;
      const height = this.container.clientHeight;
      this.canvas.width = width * dpr;
      this.canvas.height = height * dpr;
      this.gl.viewport(0, 0, this.canvas.width, this.canvas.height);
    }
    
    render = (time) => {
      if (this.disposed) return;
      
      const dt = time - this.lastTime;
      this.lastTime = time;
      this.currentFrame += dt;
      
      // Enable blending for proper transparency
      this.gl.enable(this.gl.BLEND);
      this.gl.blendFunc(this.gl.SRC_ALPHA, this.gl.ONE_MINUS_SRC_ALPHA);
      
      this.gl.clearColor(0, 0, 0, 0);
      this.gl.clear(this.gl.COLOR_BUFFER_BIT);
      this.gl.useProgram(this.program);
      this.gl.uniform1f(this.uTime, this.currentFrame * 0.001);
      this.gl.uniform2f(this.uRes, this.canvas.width, this.canvas.height);
      this.gl.drawArrays(this.gl.TRIANGLES, 0, 6);
      
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
      this.canvas.remove();
    }
  }
  
  // Public API
  return {
    create: function(containerId, size) {
      const container = document.getElementById(containerId);
      if (!container) {
        console.error('LiquidMetal: Container not found:', containerId);
        return null;
      }
      return new MiniShaderMount(container, size);
    }
  };
})();