import type { SpreadLeaf } from "./spread-fold.ts";

export interface CurlPose {
  originX: number;
  originY: number;
  axisX: number;
  axisY: number;
  radius: number;
  progress: number;
}

const VERT_MESH = `
  attribute vec2 a_uv;
  uniform vec2 u_size;
  uniform vec2 u_origin;
  uniform vec2 u_axis;
  uniform float u_radius;
  uniform vec4 u_leaf;
  uniform float u_spine;
  varying vec2 v_uv;
  varying float v_side;
  varying float v_shade;
  varying float v_roll;

  const float PI = 3.14159265;
  const float HALF_PI = 1.57079632;

  void main() {
    // Readest's leaf-only cylinder: pin the spine and keep texture coordinates
    // on the original paper while the open edge wraps around the crease.
    // Adapted from Readest pageCurl.ts (c3a95ba7, AGPL-3.0-only).
    vec2 p = u_leaf.xy + a_uv * u_leaf.zw;
    v_uv = p / u_size;
    float R = max(u_radius, 0.001);
    float d = dot(p - u_origin, u_axis);
    if (u_spine >= 0.0 && abs(p.x - u_spine) < 0.01) d = 0.0;
    vec2 pos = p;
    float z = 0.0;
    v_roll = 0.0;

    if (d <= 0.0) {
      v_side = 0.0;
      v_shade = 1.0 - 0.07 * exp(d / max(R * 1.4, 8.0));
    } else if (d < PI * R) {
      float theta = d / R;
      pos = p + u_axis * (R * sin(theta) - d);
      z = R * (1.0 - cos(theta));
      v_side = step(HALF_PI, theta);
      v_roll = sin(theta);
      float cylLight = 0.72 + 0.28 * abs(cos(theta));
      float specular = 0.09 * exp(-5.76 * (theta - HALF_PI) * (theta - HALF_PI));
      v_shade = min(1.06, cylLight + specular);
    } else {
      pos = p + u_axis * (PI * R - 2.0 * d);
      z = 2.0 * R;
      v_side = 1.0;
      v_shade = 0.88 - 0.12 * clamp((d - PI * R) / max(u_size.x * 0.65, 1.0), 0.0, 1.0);
    }

    vec2 ndc = (pos / u_size) * 2.0 - 1.0;
    ndc.y = -ndc.y;
    float maxZ = max(R * 2.2, 1.0);
    float depth = clamp(0.85 - (z / maxZ) * 0.75, -0.95, 0.95);
    gl_Position = vec4(ndc, depth, 1.0);
  }
`;

const FRAG_MESH = `
  precision mediump float;
  uniform sampler2D u_tex;
  uniform sampler2D u_incoming;
  uniform float u_backMirror;
  uniform vec3 u_paperColor;
  varying vec2 v_uv;
  varying float v_side;
  varying float v_shade;
  varying float v_roll;

  void main() {
    vec4 tex = texture2D(u_tex, v_uv);
    if (u_backMirror >= 0.0) {
      // Reflect around the spine so the destination leaf is legible on landing,
      // not mirrored like ink seen through the outgoing page.
      vec4 face = gl_FrontFacing ? tex :
        texture2D(u_incoming, vec2(u_backMirror - v_uv.x, v_uv.y));
      float lighting = 1.0 - 0.20 * v_roll;
      gl_FragColor = vec4(face.rgb * lighting, 1.0);
    } else if (v_side < 0.5) {
      gl_FragColor = vec4(tex.rgb * v_shade, 1.0);
    } else {
      vec3 backTint = mix(u_paperColor, tex.rgb, 0.18) * v_shade;
      gl_FragColor = vec4(backTint, 1.0);
    }
  }
`;

const VERT_SHADOW = `
  attribute vec2 a_uv;
  uniform vec2 u_size;
  varying vec2 v_pos;
  void main() {
    v_pos = a_uv * u_size;
    vec2 ndc = a_uv * 2.0 - 1.0;
    ndc.y = -ndc.y;
    gl_Position = vec4(ndc, 0.95, 1.0);
  }
`;

const FRAG_SHADOW = `
  precision mediump float;
  uniform vec2 u_origin;
  uniform vec2 u_axis;
  uniform float u_radius;
  uniform float u_progress;
  varying vec2 v_pos;

  void main() {
    float d = dot(v_pos - u_origin, u_axis);
    float falloff = max(u_radius * 1.6, 28.0);
    float mask = smoothstep(-4.0, 6.0, d);
    float alpha = mask * 0.24 * exp(-max(d, 0.0) / falloff) * (1.0 - smoothstep(0.88, 1.0, u_progress));
    gl_FragColor = vec4(0.0, 0.0, 0.0, alpha);
  }
`;

function compileShader(gl: WebGLRenderingContext, type: number, source: string): WebGLShader | null {
  const shader = gl.createShader(type);
  if (!shader) return null;
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    gl.deleteShader(shader);
    return null;
  }
  return shader;
}

function linkProgram(gl: WebGLRenderingContext, vsSource: string, fsSource: string): WebGLProgram | null {
  const vs = compileShader(gl, gl.VERTEX_SHADER, vsSource);
  const fs = compileShader(gl, gl.FRAGMENT_SHADER, fsSource);
  if (!vs || !fs) {
    if (vs) gl.deleteShader(vs);
    if (fs) gl.deleteShader(fs);
    return null;
  }
  const prog = gl.createProgram();
  if (!prog) {
    gl.deleteShader(vs);
    gl.deleteShader(fs);
    return null;
  }
  gl.attachShader(prog, vs);
  gl.attachShader(prog, fs);
  gl.linkProgram(prog);
  gl.deleteShader(vs);
  gl.deleteShader(fs);
  if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) {
    gl.deleteProgram(prog);
    return null;
  }
  return prog;
}

function parseRgbNormalized(cssColor: string, fallback: [number, number, number]): [number, number, number] {
  const m = cssColor.match(/rgba?\(\s*([0-9.]+)[\s,]+([0-9.]+)[\s,]+([0-9.]+)/i);
  if (m) {
    return [
      Math.min(1, parseFloat(m[1]) / 255),
      Math.min(1, parseFloat(m[2]) / 255),
      Math.min(1, parseFloat(m[3]) / 255),
    ];
  }
  const hex = cssColor.trim().replace(/^#/, "");
  if (hex.length === 6) {
    const num = parseInt(hex, 16);
    if (Number.isFinite(num)) {
      return [((num >> 16) & 255) / 255, ((num >> 8) & 255) / 255, (num & 255) / 255];
    }
  }
  return fallback;
}

/** The Canvas2D fallback has its own element: canvas context types cannot be switched. */
export class PageCurlRenderer {
  private gl: WebGLRenderingContext | null;
  private meshProgram: WebGLProgram | null = null;
  private shadowProgram: WebGLProgram | null = null;
  private meshVbo: WebGLBuffer | null = null;
  private meshIbo: WebGLBuffer | null = null;
  private shadowVbo: WebGLBuffer | null = null;
  private indexCount = 0;
  private texture: WebGLTexture | null = null;
  private incoming: WebGLTexture | null = null;
  private meshUniforms: Record<string, WebGLUniformLocation | null> = {};
  private shadowUniforms: Record<string, WebGLUniformLocation | null> = {};
  private meshAttribute = 0;
  private shadowAttribute = 0;
  private ctx2d: CanvasRenderingContext2D | null = null;
  private source: HTMLCanvasElement | null = null;
  private paper = "";
  private paperRgb: [number, number, number] = [1, 1, 1];
  private width = 1;
  private height = 1;
  private gpuSurface = false;

  constructor(
    private canvas: HTMLCanvasElement,
    private fallback: HTMLCanvasElement,
    { antialias = true } = {},
  ) {
    this.gl = canvas.getContext("webgl", { alpha: true, depth: true, antialias });
    if (this.gl && !this.initWebGL(this.gl)) this.destroy();
  }

  private initWebGL(gl: WebGLRenderingContext): boolean {
    const meshProgram = linkProgram(gl, VERT_MESH, FRAG_MESH);
    const shadowProgram = linkProgram(gl, VERT_SHADOW, FRAG_SHADOW);
    this.meshProgram = meshProgram;
    this.shadowProgram = shadowProgram;
    if (!meshProgram || !shadowProgram) return false;

    const cols = 56;
    const rows = 36;
    const verts = new Float32Array((cols + 1) * (rows + 1) * 2);
    let ptr = 0;
    for (let r = 0; r <= rows; r += 1) {
      const v = r / rows;
      for (let c = 0; c <= cols; c += 1) {
        verts[ptr++] = c / cols;
        verts[ptr++] = v;
      }
    }

    const indices: number[] = [];
    for (let r = 0; r < rows; r += 1) {
      for (let c = 0; c < cols; c += 1) {
        const i0 = r * (cols + 1) + c;
        const i1 = i0 + 1;
        const i2 = i0 + (cols + 1);
        const i3 = i2 + 1;
        indices.push(i0, i2, i1, i1, i2, i3);
      }
    }

    const meshVbo = gl.createBuffer();
    const meshIbo = gl.createBuffer();
    const shadowVbo = gl.createBuffer();
    const texture = gl.createTexture();
    this.meshVbo = meshVbo;
    this.meshIbo = meshIbo;
    this.shadowVbo = shadowVbo;
    this.texture = texture;
    if (!meshVbo || !meshIbo || !shadowVbo || !texture) return false;

    gl.bindBuffer(gl.ARRAY_BUFFER, meshVbo);
    gl.bufferData(gl.ARRAY_BUFFER, verts, gl.STATIC_DRAW);

    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, meshIbo);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, new Uint16Array(indices), gl.STATIC_DRAW);

    gl.bindBuffer(gl.ARRAY_BUFFER, shadowVbo);
    gl.bufferData(
      gl.ARRAY_BUFFER,
      new Float32Array([0, 0, 1, 0, 0, 1, 0, 1, 1, 0, 1, 1]),
      gl.STATIC_DRAW,
    );

    gl.bindTexture(gl.TEXTURE_2D, texture);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);

    this.indexCount = indices.length;
    this.meshAttribute = gl.getAttribLocation(meshProgram, "a_uv");
    this.shadowAttribute = gl.getAttribLocation(shadowProgram, "a_uv");
    for (const key of ["size", "origin", "axis", "radius", "tex", "paperColor", "leaf", "spine", "incoming", "backMirror"]) {
      this.meshUniforms[key] = gl.getUniformLocation(meshProgram, `u_${key}`);
    }
    for (const key of ["size", "origin", "axis", "radius", "progress"]) {
      this.shadowUniforms[key] = gl.getUniformLocation(shadowProgram, `u_${key}`);
    }
    return gl.getError() === gl.NO_ERROR;
  }

  setSpreadSurface(source: HTMLCanvasElement, back: HTMLCanvasElement | ImageData, w: number, h: number, paper: string) {
    // A captured viewport already has its live high-resolution page underneath.
    // Spend fewer fragments on the brief moving leaf, not on its texture.
    this.setPageSurface(source, w, h, paper, back instanceof ImageData ? 0.85 : 1);
    const gl = this.gl;
    if (!gl || !this.gpuSurface) return false;
    this.incoming ??= gl.createTexture();
    if (!this.incoming) return false;
    gl.activeTexture(gl.TEXTURE1);
    gl.bindTexture(gl.TEXTURE_2D, this.incoming);
    try {
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, back);
      return gl.getError() === gl.NO_ERROR;
    } catch (error) {
      if (!(error instanceof DOMException && error.name === "SecurityError")) throw error;
      return false;
    } finally {
      gl.activeTexture(gl.TEXTURE0);
    }
  }

  canCurlSpread(): boolean {
    return Boolean(this.gl && !this.gl.isContextLost() && this.texture);
  }

  renderSpread(pose: CurlPose, leaf: SpreadLeaf): boolean {
    if (!this.gpuSurface || !this.gl || this.gl.isContextLost()) return false;
    this.renderWebGL(pose, leaf);
    return true;
  }

  setPageSurface(source: HTMLCanvasElement, w: number, h: number, paper: string, drawScale = 1) {
    this.source = source;
    this.width = w;
    this.height = h;
    this.paper = paper;
    this.paperRgb = parseRgbNormalized(paper, [1, 1, 1]);
    this.gpuSurface = false;

    const gl = this.gl;
    if (gl && !gl.isContextLost() && source.width <= gl.getParameter(gl.MAX_TEXTURE_SIZE) &&
      source.height <= gl.getParameter(gl.MAX_TEXTURE_SIZE)) {
      this.canvas.width = Math.max(1, Math.round(source.width * drawScale));
      this.canvas.height = Math.max(1, Math.round(source.height * drawScale));
      gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, this.texture);
      try {
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, source);
        this.gpuSurface = gl.getError() === gl.NO_ERROR;
      } catch (error) {
        // A canvas with a decoded platform-protocol image may be paintable in 2D but not uploadable.
        if (!(error instanceof DOMException && error.name === "SecurityError")) throw error;
      }
    }
  }

  render(pose: CurlPose): "webgl" | "canvas2d" | null {
    if (!this.source) return null;
    if (this.gpuSurface && this.gl && !this.gl.isContextLost()) {
      this.renderWebGL(pose);
      return "webgl";
    }

    this.ctx2d ??= this.fallback.getContext("2d");
    if (!this.ctx2d) return null;
    if (this.fallback.width !== this.source.width || this.fallback.height !== this.source.height) {
      this.fallback.width = this.source.width;
      this.fallback.height = this.source.height;
    }
    this.renderFallback2D(this.ctx2d, this.source, pose);
    return "canvas2d";
  }

  releaseSurface() {
    this.source = null;
    this.gpuSurface = false;
    // Release page pixels between turns; the small mesh and programs can be reused.
    if (this.canvas.width !== 1 || this.canvas.height !== 1) {
      this.canvas.width = this.canvas.height = 1;
    }
    if (this.fallback.width !== 1 || this.fallback.height !== 1) {
      this.fallback.width = this.fallback.height = 1;
    }
    const gl = this.gl;
    if (gl && this.incoming) gl.deleteTexture(this.incoming);
    this.incoming = null;
    if (gl && !gl.isContextLost() && this.texture) {
      gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, this.texture);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 1, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
    }
  }

  private renderWebGL(pose: CurlPose, leaf?: SpreadLeaf) {
    const gl = this.gl!;
    gl.viewport(0, 0, this.canvas.width, this.canvas.height);
    gl.clearColor(0, 0, 0, 0);
    gl.clearDepth(1.0);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);

    // Pass 1: Soft cast shadow onto underneath live page
    gl.disable(gl.DEPTH_TEST);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    gl.useProgram(this.shadowProgram);

    gl.bindBuffer(gl.ARRAY_BUFFER, this.shadowVbo);
    const sAttr = this.shadowAttribute;
    gl.enableVertexAttribArray(sAttr);
    gl.vertexAttribPointer(sAttr, 2, gl.FLOAT, false, 0, 0);
    gl.uniform2f(this.shadowUniforms.size, this.width, this.height);
    gl.uniform2f(this.shadowUniforms.origin, pose.originX, pose.originY);
    gl.uniform2f(this.shadowUniforms.axis, pose.axisX, pose.axisY);
    gl.uniform1f(this.shadowUniforms.radius, pose.radius);
    gl.uniform1f(this.shadowUniforms.progress, pose.progress);
    gl.drawArrays(gl.TRIANGLES, 0, 6);

    // Pass 2: 3D cylindrical curl mesh
    gl.enable(gl.DEPTH_TEST);
    gl.depthFunc(gl.LEQUAL);
    gl.disable(gl.BLEND);
    gl.disable(gl.CULL_FACE);
    gl.useProgram(this.meshProgram);

    gl.bindBuffer(gl.ARRAY_BUFFER, this.meshVbo);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, this.meshIbo);
    const mAttr = this.meshAttribute;
    gl.enableVertexAttribArray(mAttr);
    gl.vertexAttribPointer(mAttr, 2, gl.FLOAT, false, 0, 0);

    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    gl.uniform1i(this.meshUniforms.tex, 0);
    gl.uniform2f(this.meshUniforms.size, this.width, this.height);
    const spine = leaf ? leaf.left + (leaf.direction < 0 ? leaf.width : 0) : -1;
    gl.uniform4f(this.meshUniforms.leaf, leaf?.left ?? 0, leaf?.top ?? 0,
      leaf?.width ?? this.width, leaf?.height ?? this.height);
    gl.uniform1f(this.meshUniforms.spine, spine);
    gl.uniform1f(this.meshUniforms.backMirror, 2 * spine / this.width);
    gl.uniform1i(this.meshUniforms.incoming, leaf ? 1 : 0);
    gl.frontFace(gl.CCW);
    gl.uniform2f(this.meshUniforms.origin, pose.originX, pose.originY);
    gl.uniform2f(this.meshUniforms.axis, pose.axisX, pose.axisY);
    gl.uniform1f(this.meshUniforms.radius, pose.radius);
    gl.uniform3f(
      this.meshUniforms.paperColor,
      this.paperRgb[0],
      this.paperRgb[1],
      this.paperRgb[2],
    );

    gl.drawElements(gl.TRIANGLES, this.indexCount, gl.UNSIGNED_SHORT, 0);
  }

  private renderFallback2D(ctx: CanvasRenderingContext2D, src: HTMLCanvasElement, pose: CurlPose) {
    const w = this.width;
    const h = this.height;
    ctx.setTransform(src.width / w, 0, 0, src.height / h, 0, 0);
    ctx.clearRect(0, 0, w, h);

    // Approximate the cylinder with a flat front, a tinted reflected strip, and a soft shadow.
    const fold = pose.originX;
    const slope = -pose.axisY / pose.axisX;
    const top = fold - pose.originY * slope;
    const bottom = fold + (h - pose.originY) * slope;
    const band = pose.radius;
    const fade = 1 - Math.max(0, (pose.progress - 0.88) / 0.12);
    function clip(left: number, right: number) {
      ctx.beginPath();
      ctx.moveTo(top + left, 0);
      ctx.lineTo(top + right, 0);
      ctx.lineTo(bottom + right, h);
      ctx.lineTo(bottom + left, h);
      ctx.closePath();
      ctx.clip();
    }

    ctx.save();
    clip(-w - band, 0);
    ctx.drawImage(src, 0, 0, w, h);
    ctx.restore();

    ctx.save();
    clip(0, band * 3);
    ctx.transform(1, 0, slope, 1, top, 0);
    const shadow = ctx.createLinearGradient(0, 0, band * 3, 0);
    shadow.addColorStop(0, `rgba(0,0,0,${0.24 * fade})`);
    shadow.addColorStop(1, "transparent");
    ctx.fillStyle = shadow;
    ctx.fillRect(0, 0, band * 3, h);
    ctx.restore();

    // No paper extends beyond the actual source edge at the start of the turn.
    const flap = Math.min(band, Math.max(0, w - Math.min(top, bottom)), Math.max(0, Math.max(top, bottom)));
    if (flap < 0.5) return;
    ctx.save();
    clip(-flap, 0);
    ctx.fillStyle = this.paper;
    ctx.fillRect(0, 0, w, h);
    ctx.globalAlpha = 0.18;
    ctx.transform(-1, 0, 2 * slope, 1, 2 * top, 0);
    ctx.drawImage(src, 0, 0, w, h);
    ctx.restore();
  }

  destroy() {
    const gl = this.gl;
    if (!gl) return;
    if (this.meshVbo) gl.deleteBuffer(this.meshVbo);
    if (this.meshIbo) gl.deleteBuffer(this.meshIbo);
    if (this.shadowVbo) gl.deleteBuffer(this.shadowVbo);
    if (this.texture) gl.deleteTexture(this.texture);
    if (this.incoming) gl.deleteTexture(this.incoming);
    if (this.meshProgram) gl.deleteProgram(this.meshProgram);
    if (this.shadowProgram) gl.deleteProgram(this.shadowProgram);
    this.gl = null;
  }
}
