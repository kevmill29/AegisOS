import * as THREE from 'three';

// The sphere is one shader-displaced icosahedron plus a slightly larger
// back-face "atmosphere" shell for the glow halo. All motion is driven by four
// smoothed scalars (time, audio level, sleep factor, link factor) so state
// transitions are always eased in the render loop rather than snapping when a
// TCP event lands.

// Compact 3D simplex noise (Ashima/webgl-noise, MIT) — the standard GLSL snippet.
const NOISE_GLSL = /* glsl */ `
vec3 mod289(vec3 x){return x-floor(x*(1.0/289.0))*289.0;}
vec4 mod289(vec4 x){return x-floor(x*(1.0/289.0))*289.0;}
vec4 permute(vec4 x){return mod289(((x*34.0)+1.0)*x);}
vec4 taylorInvSqrt(vec4 r){return 1.79284291400159-0.85373472095314*r;}
float snoise(vec3 v){
  const vec2 C=vec2(1.0/6.0,1.0/3.0);
  const vec4 D=vec4(0.0,0.5,1.0,2.0);
  vec3 i=floor(v+dot(v,C.yyy));
  vec3 x0=v-i+dot(i,C.xxx);
  vec3 g=step(x0.yzx,x0.xyz);
  vec3 l=1.0-g;
  vec3 i1=min(g.xyz,l.zxy);
  vec3 i2=max(g.xyz,l.zxy);
  vec3 x1=x0-i1+C.xxx;
  vec3 x2=x0-i2+C.yyy;
  vec3 x3=x0-D.yyy;
  i=mod289(i);
  vec4 p=permute(permute(permute(i.z+vec4(0.0,i1.z,i2.z,1.0))+i.y+vec4(0.0,i1.y,i2.y,1.0))+i.x+vec4(0.0,i1.x,i2.x,1.0));
  float n_=0.142857142857;
  vec3 ns=n_*D.wyz-D.xzx;
  vec4 j=p-49.0*floor(p*ns.z*ns.z);
  vec4 x_=floor(j*ns.z);
  vec4 y_=floor(j-7.0*x_);
  vec4 x=x_*ns.x+ns.yyyy;
  vec4 y=y_*ns.x+ns.yyyy;
  vec4 h=1.0-abs(x)-abs(y);
  vec4 b0=vec4(x.xy,y.xy);
  vec4 b1=vec4(x.zw,y.zw);
  vec4 s0=floor(b0)*2.0+1.0;
  vec4 s1=floor(b1)*2.0+1.0;
  vec4 sh=-step(h,vec4(0.0));
  vec4 a0=b0.xzyw+s0.xzyw*sh.xxyy;
  vec4 a1=b1.xzyw+s1.xzyw*sh.zzww;
  vec3 p0=vec3(a0.xy,h.x);
  vec3 p1=vec3(a0.zw,h.y);
  vec3 p2=vec3(a1.xy,h.z);
  vec3 p3=vec3(a1.zw,h.w);
  vec4 norm=taylorInvSqrt(vec4(dot(p0,p0),dot(p1,p1),dot(p2,p2),dot(p3,p3)));
  p0*=norm.x;p1*=norm.y;p2*=norm.z;p3*=norm.w;
  vec4 m=max(0.6-vec4(dot(x0,x0),dot(x1,x1),dot(x2,x2),dot(x3,x3)),0.0);
  m=m*m;
  return 42.0*dot(m*m,vec4(dot(p0,x0),dot(p1,x1),dot(p2,x2),dot(p3,x3)));
}
`;

const CORE_VERTEX = /* glsl */ `
${NOISE_GLSL}
uniform float uTime;
uniform float uAudio;
uniform float uSleep;
varying vec3 vNormal;
varying float vDisplace;
void main() {
  // Sleep slows the noise field and flattens displacement: dormant = near-still.
  float speed = mix(0.35, 0.06, uSleep);
  float amp = mix(0.10 + 0.35 * uAudio, 0.02, uSleep);
  float n = snoise(normal * 1.8 + vec3(uTime * speed));
  vDisplace = n;
  vec3 displaced = position + normal * n * amp;
  vNormal = normalize(normalMatrix * normal);
  gl_Position = projectionMatrix * modelViewMatrix * vec4(displaced, 1.0);
}
`;

const CORE_FRAGMENT = /* glsl */ `
uniform float uAudio;
uniform float uSleep;
uniform float uLink;
varying vec3 vNormal;
varying float vDisplace;
void main() {
  // Awake: cyan. Sleeping: dim deep blue. Link lost: drift toward amber warning.
  // ("awake" not "active": 'active' is a reserved word in GLSL ES — desktop
  // drivers tolerate it, ANGLE/SwiftShader correctly rejects the shader.)
  vec3 awake = vec3(0.20, 0.85, 1.00);
  vec3 asleep = vec3(0.10, 0.16, 0.38);
  vec3 warning = vec3(1.00, 0.55, 0.15);
  vec3 base = mix(awake, asleep, uSleep);
  base = mix(warning, base, uLink);

  float fresnel = pow(1.0 - abs(dot(normalize(vNormal), vec3(0.0, 0.0, 1.0))), 2.0);
  float energy = mix(0.9, 0.25, uSleep) + uAudio * 0.8;
  vec3 color = base * (0.35 + fresnel * 1.4) * energy;
  color += base * smoothstep(0.4, 1.0, vDisplace) * 0.6 * (1.0 - uSleep);
  gl_FragColor = vec4(color, 1.0);
}
`;

const HALO_VERTEX = /* glsl */ `
varying vec3 vNormal;
void main() {
  vNormal = normalize(normalMatrix * normal);
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
`;

const HALO_FRAGMENT = /* glsl */ `
uniform float uSleep;
uniform float uLink;
varying vec3 vNormal;
void main() {
  vec3 awake = vec3(0.20, 0.85, 1.00);
  vec3 asleep = vec3(0.10, 0.16, 0.38);
  vec3 warning = vec3(1.00, 0.55, 0.15);
  vec3 base = mix(warning, mix(awake, asleep, uSleep), uLink);
  // Rendered on back faces only, so this is pure rim: strongest at the silhouette.
  float rim = pow(0.72 - dot(normalize(vNormal), vec3(0.0, 0.0, 1.0)), 3.5);
  gl_FragColor = vec4(base, 1.0) * rim * mix(1.0, 0.3, uSleep);
}
`;

// Full-canvas ambient background: a faint, slow "wispy cloud" of the sphere's
// own light. It peaks around the sphere and falls off with distance, so on a
// single screen it reads as a gentle atmosphere and on an EXTENDED multi-monitor
// desktop the sphere sits on the primary screen while its light spills across
// the others as drifting cloud — instead of the sphere splitting on the seam.
const BG_VERTEX = /* glsl */ `
varying vec2 vUv;
void main() {
  vUv = uv;
  gl_Position = vec4(position.xy, 0.0, 1.0);
}
`;

const BG_FRAGMENT = /* glsl */ `
${NOISE_GLSL}
uniform float uTime;
uniform float uAudio;
uniform float uSleep;
uniform float uLink;
uniform float uAspect;   // canvas width/height, so distance is round not oval
uniform vec2  uFocus;    // where the sphere sits on the canvas, in UV (0..1)
varying vec2 vUv;

float fbm(vec3 p) {
  float f = 0.0, a = 0.5;
  for (int i = 0; i < 3; i++) { f += a * snoise(p); p *= 2.02; a *= 0.5; }
  return f;
}

void main() {
  vec3 awake = vec3(0.20, 0.85, 1.00);
  vec3 asleep = vec3(0.10, 0.16, 0.38);
  vec3 warning = vec3(1.00, 0.55, 0.15);
  vec3 base = mix(warning, mix(awake, asleep, uSleep), uLink);

  // Aspect-correct distance from the sphere's screen position.
  vec2 d = vUv - uFocus;
  d.x *= uAspect;
  float dist = length(d);

  // Slow, domain-warped cloud.
  vec3 q = vec3(vUv * 2.4, uTime * 0.025);
  float cloud = fbm(q + 0.6 * fbm(q * 0.5)) * 0.5 + 0.5;

  // Bright near the sphere, fading outward — this is what spills onto screen 2.
  float halo = exp(-dist * 1.7);
  float energy = mix(0.45, 0.16, uSleep) + uAudio * 0.5;
  float intensity = halo * (0.30 + 0.70 * cloud) * energy;

  gl_FragColor = vec4(base * intensity * 0.6, 1.0);
}
`;

function lerp(a, b, t) {
  return a + (b - a) * t;
}

export class AegisSphere {
  constructor(canvas) {
    this.renderer = new THREE.WebGLRenderer({ canvas, antialias: true });
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    // We draw the ambient cloud then the sphere over it, so clear by hand.
    this.renderer.autoClear = false;

    this.scene = new THREE.Scene();
    this.camera = new THREE.PerspectiveCamera(45, 1, 0.1, 100);
    this.camera.position.z = 4.2;

    // Where the sphere should sit on the canvas, 0..1 (0.5 = centered). On an
    // extended desktop this is the primary display's center, so the sphere lands
    // on one screen and the cloud fills the rest.
    this.focusX = 0.5;
    this.uAspect = { value: 1 };
    this.uFocus = { value: new THREE.Vector2(0.5, 0.5) };

    this.uniforms = {
      uTime: { value: 0 },
      uAudio: { value: 0 },
      uSleep: { value: 0 },
      uLink: { value: 0 },
    };

    // Ambient cloud: a fullscreen quad drawn with an orthographic camera before
    // the sphere. Shares the animated uniforms so it eases with the sphere state.
    this.bgScene = new THREE.Scene();
    this.bgCamera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
    this.bgScene.add(
      new THREE.Mesh(
        new THREE.PlaneGeometry(2, 2),
        new THREE.ShaderMaterial({
          vertexShader: BG_VERTEX,
          fragmentShader: BG_FRAGMENT,
          uniforms: {
            uTime: this.uniforms.uTime,
            uAudio: this.uniforms.uAudio,
            uSleep: this.uniforms.uSleep,
            uLink: this.uniforms.uLink,
            uAspect: this.uAspect,
            uFocus: this.uFocus,
          },
          depthTest: false,
          depthWrite: false,
        })
      )
    );

    const core = new THREE.Mesh(
      new THREE.IcosahedronGeometry(1, 48),
      new THREE.ShaderMaterial({
        vertexShader: CORE_VERTEX,
        fragmentShader: CORE_FRAGMENT,
        uniforms: this.uniforms,
      })
    );

    const halo = new THREE.Mesh(
      new THREE.IcosahedronGeometry(1.35, 24),
      new THREE.ShaderMaterial({
        vertexShader: HALO_VERTEX,
        fragmentShader: HALO_FRAGMENT,
        uniforms: this.uniforms,
        side: THREE.BackSide,
        blending: THREE.AdditiveBlending,
        transparent: true,
        depthWrite: false,
      })
    );

    this.group = new THREE.Group();
    this.group.add(core, halo);
    this.scene.add(this.group);

    // Raw targets set from events/audio; uniforms chase these each frame.
    this.targets = { audio: 0, sleep: 0, link: 0 };
    this.clock = new THREE.Clock();
    this.running = true;

    this.resize();
    this._onResize = () => this.resize();
    window.addEventListener('resize', this._onResize);
    this.renderer.setAnimationLoop(() => this.frame());
  }

  setAudioLevel(level) {
    this.targets.audio = level;
  }

  setSleeping(sleeping) {
    this.targets.sleep = sleeping ? 1 : 0;
  }

  setLinkConnected(connected) {
    this.targets.link = connected ? 1 : 0;
  }

  frame() {
    const dt = Math.min(this.clock.getDelta(), 0.1);
    const u = this.uniforms;
    u.uTime.value += dt;
    // Audio tracks fast (it *is* the reactivity); mode changes ease over ~a second.
    u.uAudio.value = lerp(u.uAudio.value, this.targets.audio, 1 - Math.exp(-dt * 12));
    u.uSleep.value = lerp(u.uSleep.value, this.targets.sleep, 1 - Math.exp(-dt * 2.5));
    u.uLink.value = lerp(u.uLink.value, this.targets.link, 1 - Math.exp(-dt * 2.5));

    const sleepScale = lerp(1.0, 0.72, u.uSleep.value);
    this.group.scale.setScalar(sleepScale);
    this.group.rotation.y += dt * lerp(0.15, 0.03, u.uSleep.value);

    this.renderer.clear();
    this.renderer.render(this.bgScene, this.bgCamera);
    this.renderer.render(this.scene, this.camera);
  }

  // focusX in 0..1 is where the sphere should sit horizontally on the canvas.
  // Windows-style extend: the sphere goes on the primary screen and its light
  // clouds the rest. Called with the primary display's center from Electron.
  setFocus(focusX) {
    this.focusX = Math.min(1, Math.max(0, focusX));
    this.applyFocus();
  }

  applyFocus() {
    const aspect = this.camera.aspect || 1;
    this.uFocus.value.set(this.focusX, 0.5);
    // Move the sphere in world X so it projects to focusX. Half the visible
    // width at the sphere's depth is tan(fov/2) * distance * aspect.
    const halfW = Math.tan((this.camera.fov * Math.PI) / 360) * this.camera.position.z * aspect;
    const ndcX = this.focusX * 2 - 1;
    this.group.position.x = ndcX * halfW;
  }

  resize() {
    const { clientWidth, clientHeight } = this.renderer.domElement.parentElement;
    this.renderer.setSize(clientWidth, clientHeight, false);
    this.camera.aspect = clientWidth / clientHeight;
    this.camera.updateProjectionMatrix();
    this.uAspect.value = this.camera.aspect;
    this.applyFocus();
  }

  dispose() {
    this.renderer.setAnimationLoop(null);
    window.removeEventListener('resize', this._onResize);
    this.scene.traverse((obj) => {
      obj.geometry?.dispose();
      obj.material?.dispose();
    });
    this.renderer.dispose();
  }
}
