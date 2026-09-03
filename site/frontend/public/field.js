// The ambient field, spec 24. A full-viewport WebGL quad running classic Perlin
// noise in three dimensions, with time added to all three components so the
// field evolves in place instead of sliding past. That evolution is the whole
// point: a translated shape reads as a moving picture, a noise field walked
// through time reads as weather.
//
// The maths is the reference's, read off luma.com's own bundle on 2 September
// 2026 (chunk 3xyg14dl5aicq.js, its `shader` base class, the theme they label
// Grain): a horizontal two-colour ramp with a third colour mixed in by the
// noise, at density 1, speed 0.3 and strength 7. Strength 7 against a noise
// term of at most 0.75 drives mix() far past its endpoints, and that
// extrapolation is what makes the third colour bloom instead of tint. The
// colours are ours and obey the ground laws: gold only on cover, red only on
// paper.
//
// Raw WebGL rather than a library: the whole program is one quad and one
// fragment shader, and a scene graph would be 600KB to draw a rectangle.
//
// Floors. Under reduced motion the field paints exactly one frame and the loop
// never starts. With the tab hidden the loop stops. Without a WebGL context
// nothing is inserted and the CSS field in styles.css stays as it is, which is
// why that CSS is still there and still correct on its own.
(function () {
  "use strict";

  var VERT =
    "attribute vec2 aPos;varying vec2 vUv;void main(){vUv=aPos*0.5+0.5;gl_Position=vec4(aPos,0.0,1.0);}";

  // Classic Perlin noise, Stefan Gustavson's cnoise, unchanged.
  var FRAG = [
    "precision highp float;",
    "uniform float uTime;uniform float uSpeed;uniform float uNoiseDensity;",
    "uniform float uNoiseStrength;uniform float uBrightness;uniform float uAlpha;",
    "uniform vec3 uColor1;uniform vec3 uColor2;uniform vec3 uColor3;",
    "uniform vec2 uAspectRatio;uniform vec2 uOffset;",
    "varying vec2 vUv;",
    "vec3 mod289(vec3 x){return x-floor(x*(1.0/289.0))*289.0;}",
    "vec4 mod289(vec4 x){return x-floor(x*(1.0/289.0))*289.0;}",
    "vec4 permute(vec4 x){return mod289(((x*34.0)+1.0)*x);}",
    "vec4 taylorInvSqrt(vec4 r){return 1.79284291400159-0.85373472095314*r;}",
    "vec3 fade(vec3 t){return t*t*t*(t*(t*6.0-15.0)+10.0);}",
    "float cnoise(vec3 P){",
    "vec3 Pi0=floor(P);vec3 Pi1=Pi0+vec3(1.0);Pi0=mod289(Pi0);Pi1=mod289(Pi1);",
    "vec3 Pf0=fract(P);vec3 Pf1=Pf0-vec3(1.0);",
    "vec4 ix=vec4(Pi0.x,Pi1.x,Pi0.x,Pi1.x);vec4 iy=vec4(Pi0.yy,Pi1.yy);",
    "vec4 iz0=Pi0.zzzz;vec4 iz1=Pi1.zzzz;",
    "vec4 ixy=permute(permute(ix)+iy);vec4 ixy0=permute(ixy+iz0);vec4 ixy1=permute(ixy+iz1);",
    "vec4 gx0=ixy0*(1.0/7.0);vec4 gy0=fract(floor(gx0)*(1.0/7.0))-0.5;gx0=fract(gx0);",
    "vec4 gz0=vec4(0.5)-abs(gx0)-abs(gy0);vec4 sz0=step(gz0,vec4(0.0));",
    "gx0-=sz0*(step(0.0,gx0)-0.5);gy0-=sz0*(step(0.0,gy0)-0.5);",
    "vec4 gx1=ixy1*(1.0/7.0);vec4 gy1=fract(floor(gx1)*(1.0/7.0))-0.5;gx1=fract(gx1);",
    "vec4 gz1=vec4(0.5)-abs(gx1)-abs(gy1);vec4 sz1=step(gz1,vec4(0.0));",
    "gx1-=sz1*(step(0.0,gx1)-0.5);gy1-=sz1*(step(0.0,gy1)-0.5);",
    "vec3 g000=vec3(gx0.x,gy0.x,gz0.x);vec3 g100=vec3(gx0.y,gy0.y,gz0.y);",
    "vec3 g010=vec3(gx0.z,gy0.z,gz0.z);vec3 g110=vec3(gx0.w,gy0.w,gz0.w);",
    "vec3 g001=vec3(gx1.x,gy1.x,gz1.x);vec3 g101=vec3(gx1.y,gy1.y,gz1.y);",
    "vec3 g011=vec3(gx1.z,gy1.z,gz1.z);vec3 g111=vec3(gx1.w,gy1.w,gz1.w);",
    "vec4 norm0=taylorInvSqrt(vec4(dot(g000,g000),dot(g010,g010),dot(g100,g100),dot(g110,g110)));",
    "g000*=norm0.x;g010*=norm0.y;g100*=norm0.z;g110*=norm0.w;",
    "vec4 norm1=taylorInvSqrt(vec4(dot(g001,g001),dot(g011,g011),dot(g101,g101),dot(g111,g111)));",
    "g001*=norm1.x;g011*=norm1.y;g101*=norm1.z;g111*=norm1.w;",
    "float n000=dot(g000,Pf0);float n100=dot(g100,vec3(Pf1.x,Pf0.yz));",
    "float n010=dot(g010,vec3(Pf0.x,Pf1.y,Pf0.z));float n110=dot(g110,vec3(Pf1.xy,Pf0.z));",
    "float n001=dot(g001,vec3(Pf0.xy,Pf1.z));float n101=dot(g101,vec3(Pf1.x,Pf0.y,Pf1.z));",
    "float n011=dot(g011,vec3(Pf0.x,Pf1.yz));float n111=dot(g111,Pf1);",
    "vec3 f=fade(Pf0);",
    "vec4 n_z=mix(vec4(n000,n100,n010,n110),vec4(n001,n101,n011,n111),f.z);",
    "vec2 n_yz=mix(n_z.xy,n_z.zw,f.y);",
    "return 2.2*mix(n_yz.x,n_yz.y,f.x);}",
    "void main(){",
    "vec2 uv=vUv;",
    "uv-=vec2(0.5);uv*=uAspectRatio;uv+=vec2(0.5);",
    "uv=(uv*5.0-2.5);",
    "uv+=uOffset;",
    "float t=uTime*uSpeed;",
    "float distortion=0.75*cnoise(0.43*vec3(uv,0.0)*uNoiseDensity+t);",
    "vec3 color=mix(uColor1,uColor2,smoothstep(-3.0,3.0,uv.x));",
    "color=mix(color,uColor3,distortion*uNoiseStrength);",
    "color*=uBrightness;color*=0.8;",
    "gl_FragColor=vec4(color,uAlpha);}"
  ].join("\n");

  function hexToRgb(hex) {
    hex = String(hex).trim().replace("#", "");
    if (hex.length === 3) hex = hex[0] + hex[0] + hex[1] + hex[1] + hex[2] + hex[2];
    var n = parseInt(hex, 16);
    if (isNaN(n)) return [0, 0, 0];
    return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
  }

  function readVars() {
    var s = getComputedStyle(document.documentElement);
    function v(name, fallback) {
      var out = s.getPropertyValue(name).trim();
      return out || fallback;
    }
    return {
      c1: hexToRgb(v("--shader-1", "#14110c")),
      c2: hexToRgb(v("--shader-2", "#241d12")),
      c3: hexToRgb(v("--shader-3", "#c9a860")),
      brightness: parseFloat(v("--shader-brightness", "1")),
      alpha: parseFloat(v("--shader-alpha", "0.55"))
    };
  }

  function compile(gl, type, src) {
    var sh = gl.createShader(type);
    gl.shaderSource(sh, src);
    gl.compileShader(sh);
    if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) return null;
    return sh;
  }

  function start() {
    var canvas = document.createElement("canvas");
    canvas.className = "field-canvas";
    canvas.setAttribute("aria-hidden", "true");

    var gl =
      canvas.getContext("webgl", { alpha: true, antialias: false, depth: false, premultipliedAlpha: false }) ||
      canvas.getContext("experimental-webgl", { alpha: true, antialias: false, depth: false });
    if (!gl) return; // No context: the CSS field in styles.css stands on its own.

    var vs = compile(gl, gl.VERTEX_SHADER, VERT);
    var fs = compile(gl, gl.FRAGMENT_SHADER, FRAG);
    if (!vs || !fs) return;
    var prog = gl.createProgram();
    gl.attachShader(prog, vs);
    gl.attachShader(prog, fs);
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) return;
    gl.useProgram(prog);

    // One triangle covering the viewport. Two would draw a seam down the middle
    // on some drivers and cost a vertex for nothing.
    var buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
    var aPos = gl.getAttribLocation(prog, "aPos");
    gl.enableVertexAttribArray(aPos);
    gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 0, 0);

    var U = {};
    ["uTime", "uSpeed", "uNoiseDensity", "uNoiseStrength", "uBrightness", "uAlpha",
     "uColor1", "uColor2", "uColor3", "uAspectRatio", "uOffset"].forEach(function (n) {
      U[n] = gl.getUniformLocation(prog, n);
    });

    // The reference's numbers, unchanged. Only the colours and the alpha are ours.
    gl.uniform1f(U.uSpeed, 0.3);
    gl.uniform1f(U.uNoiseDensity, 1.0);
    gl.uniform1f(U.uNoiseStrength, 7.0);
    gl.uniform2f(U.uOffset, 0.0, 0.0);

    var vars = readVars();
    function applyColours() {
      vars = readVars();
      gl.uniform3fv(U.uColor1, vars.c1);
      gl.uniform3fv(U.uColor2, vars.c2);
      gl.uniform3fv(U.uColor3, vars.c3);
      gl.uniform1f(U.uBrightness, vars.brightness);
      gl.uniform1f(U.uAlpha, vars.alpha);
    }

    // Half resolution, capped at 1 device pixel per CSS pixel. The field is all
    // low frequency, so nobody can see the difference, and it quarters the
    // fragment work on a phone.
    function resize() {
      var dpr = Math.min(window.devicePixelRatio || 1, 1);
      var w = Math.max(1, Math.round(window.innerWidth * dpr * 0.5));
      var h = Math.max(1, Math.round(window.innerHeight * dpr * 0.5));
      if (canvas.width === w && canvas.height === h) return;
      canvas.width = w;
      canvas.height = h;
      gl.viewport(0, 0, w, h);
      gl.uniform2f(U.uAspectRatio, w > h ? 1 : w / h, w > h ? h / w : 1);
    }

    function draw(seconds) {
      gl.uniform1f(U.uTime, seconds);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
    }

    document.documentElement.classList.add("has-field");
    document.body.appendChild(canvas);
    applyColours();
    resize();

    var reduce = matchMedia("(prefers-reduced-motion: reduce)");
    var running = false;
    var t0 = null;

    function frame(now) {
      if (!running) return;
      if (t0 === null) t0 = now;
      resize();
      draw((now - t0) / 1000);
      requestAnimationFrame(frame);
    }

    function play() {
      if (running || reduce.matches || document.hidden) return;
      running = true;
      requestAnimationFrame(frame);
    }

    function stop() {
      running = false;
    }

    if (reduce.matches) {
      // One composed frame, far enough into the field to be interesting, and
      // then nothing moves for the rest of the visit.
      resize();
      draw(40);
    } else {
      play();
    }

    document.addEventListener("visibilitychange", function () {
      if (document.hidden) stop();
      else { t0 = null; play(); }
    });
    window.addEventListener("resize", function () {
      resize();
      if (!running) draw(40);
    });
    if (reduce.addEventListener) {
      reduce.addEventListener("change", function () {
        if (reduce.matches) { stop(); resize(); draw(40); } else { t0 = null; play(); }
      });
    }

    // The lamp flips data-theme in place, so the colours have to follow it
    // without a reload.
    new MutationObserver(function () {
      applyColours();
      if (!running) draw(40);
    }).observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", start);
  } else {
    start();
  }
})();
