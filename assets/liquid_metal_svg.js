// Liquid Metal SVG Component - Applies liquid metal shader to SVG shapes
window.LiquidMetalSVG = (function() {
  'use strict';

  class SVGShaderMount {
    constructor(container, svgPath, size) {
      this.container = container;
      this.svgPath = svgPath;
      this.size = size || 48;
      this.canvas = null;
      this.gl = null;
      this.maskTexture = null;
      this.disposed = false;
      this.rafId = null;
      this.currentFrame = 0;
      this.lastTime = 0;
      
      this.loadSVG();
    }
    
    async loadSVG() {
      console.log('🎨 [SVG] Loading SVG from:', this.svgPath);
      
      try {
        // Fetch SVG as text
        console.log('🔄 [SVG] Fetching SVG content...');
        const response = await fetch(this.svgPath);
        if (!response.ok) {
          throw new Error(`HTTP error! status: ${response.status}`);
        }
        
        const svgText = await response.text();
        console.log('✅ [SVG] SVG content fetched, length:', svgText.length);
        
        // Convert to data URL
        const svgBlob = new Blob([svgText], { type: 'image/svg+xml;charset=utf-8' });
        const svgDataUrl = URL.createObjectURL(svgBlob);
        console.log('🔄 [SVG] Created blob URL for SVG');
        
        // Load into Image element
        const img = new Image();
        img.onload = () => {
          console.log('✅ [SVG] Image loaded successfully');
          console.log('📐 [SVG] Image dimensions:', img.width, 'x', img.height);
          URL.revokeObjectURL(svgDataUrl); // Clean up blob URL
          this.initCanvas();
          this.createMaskTexture(img);
          this.initShader();
          this.start();
          console.log('✅ [SVG] Complete initialization finished');
        };
        
        img.onerror = (e) => {
          console.error('❌ [SVG] Failed to load image from blob URL');
          console.error('❌ [SVG] Error:', e);
          URL.revokeObjectURL(svgDataUrl);
        };
        
        img.src = svgDataUrl;
        
      } catch (error) {
        console.error('❌ [SVG] Failed to fetch SVG:', error);
      }
    }
    
    initCanvas() {
      console.log('🎨 [Canvas] Initializing canvas for container:', this.container.id);
      this.canvas = document.createElement('canvas');
      this.canvas.style.cssText = 'width:100%;height:100%;display:block;';
      this.container.appendChild(this.canvas);
      console.log('✅ [Canvas] Canvas appended to container');
      
      this.gl = this.canvas.getContext('webgl2', { 
        alpha: true, 
        premultipliedAlpha: false,
        antialias: true
      });
      
      if (!this.gl) {
        console.error('❌ [Canvas] WebGL2 not supported');
        return;
      }
      console.log('✅ [Canvas] WebGL2 context created');
      
      this.resize();
      
      // Handle container resize
      this.resizeObserver = new ResizeObserver(() => this.resize());
      this.resizeObserver.observe(this.container);
      console.log('✅ [Canvas] Resize observer attached');
    }
    
    createMaskTexture(img) {
      console.log('🎭 [Mask] Creating mask texture from SVG');
      const gl = this.gl;
      
      // Create a canvas to rasterize the SVG
      const tempCanvas = document.createElement('canvas');
      tempCanvas.width = this.size * 2;
      tempCanvas.height = this.size * 2;
      console.log('📐 [Mask] Temp canvas size:', tempCanvas.width, 'x', tempCanvas.height);
      
      const tempCtx = tempCanvas.getContext('2d');
      
      // Draw SVG to canvas
      tempCtx.clearRect(0, 0, tempCanvas.width, tempCanvas.height);
      tempCtx.drawImage(img, 0, 0, tempCanvas.width, tempCanvas.height);
      console.log('✅ [Mask] SVG drawn to temp canvas');
      
      // Create WebGL texture from canvas
      this.maskTexture = gl.createTexture();
      gl.bindTexture(gl.TEXTURE_2D, this.maskTexture);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, tempCanvas);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
      
      console.log('✅ [Mask] WebGL texture created and configured');
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
        uniform sampler2D u_mask;
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
          // Convert from clip space to texture coordinates
          vec2 texCoord = v_uv * 0.5 + 0.5;
          texCoord.y = 1.0 - texCoord.y;
          
          // Sample the SVG mask
          vec4 maskColor = texture(u_mask, texCoord);
          float maskAlpha = maskColor.a;
          
          // Discard pixels outside the SVG shape
          if (maskAlpha < 0.01) {
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
          
          // Use mask alpha for smooth edges
          fragColor = vec4(color, maskAlpha);
        }
      `;
      
      const vs = gl.createShader(gl.VERTEX_SHADER);
      gl.shaderSource(vs, vertexShader);
      gl.compileShader(vs);
      
      const fs = gl.createShader(gl.FRAGMENT_SHADER);
      gl.shaderSource(fs, fragmentShader);
      gl.compileShader(fs);
      
      this.program = gl.createProgram();
      gl.attachShader(this.program, vs);
      gl.attachShader(this.program, fs);
      gl.linkProgram(this.program);
      
      const posLoc = gl.getAttribLocation(this.program, 'a_position');
      const posBuf = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, posBuf);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1,-1, 1,-1, -1,1, -1,1, 1,-1, 1,1]), gl.STATIC_DRAW);
      gl.enableVertexAttribArray(posLoc);
      gl.vertexAttribPointer(posLoc, 2, gl.FLOAT, false, 0, 0);
      
      this.uTime = gl.getUniformLocation(this.program, 'u_time');
      this.uRes = gl.getUniformLocation(this.program, 'u_resolution');
      this.uMask = gl.getUniformLocation(this.program, 'u_mask');
      
      console.log('✅ [Shader] SVG shader program created and linked');
      console.log('📍 [Shader] Uniform locations - time:', this.uTime, 'res:', this.uRes, 'mask:', this.uMask);
    }
    
    resize() {
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      const width = this.container.clientWidth;
      const height = this.container.clientHeight;
      this.canvas.width = width * dpr;
      this.canvas.height = height * dpr;
      this.gl.viewport(0, 0, this.canvas.width, this.canvas.height);
      console.log('📏 [Resize] Canvas resized to:', this.canvas.width, 'x', this.canvas.height, '(DPR:', dpr + ')');
    }
    
    render = (time) => {
      if (this.disposed) return;
      
      const dt = time - this.lastTime;
      this.lastTime = time;
      this.currentFrame += dt;
      
      const gl = this.gl;
      
      gl.enable(gl.BLEND);
      gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
      
      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT);
      gl.useProgram(this.program);
      
      // Bind mask texture
      gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, this.maskTexture);
      gl.uniform1i(this.uMask, 0);
      
      gl.uniform1f(this.uTime, this.currentFrame * 0.001);
      gl.uniform2f(this.uRes, this.canvas.width, this.canvas.height);
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
      if (this.gl) {
        if (this.program) this.gl.deleteProgram(this.program);
        if (this.maskTexture) this.gl.deleteTexture(this.maskTexture);
      }
      if (this.canvas) this.canvas.remove();
    }
  }
  
  // Public API
  return {
    create: function(containerId, svgPath, size) {
      const container = document.getElementById(containerId);
      if (!container) {
        console.error('LiquidMetalSVG: Container not found:', containerId);
        return null;
      }
      return new SVGShaderMount(container, svgPath, size);
    }
  };
})();