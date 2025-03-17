precision mediump float;

varying vec3 vUv;
uniform samplerCube uSampler;

void main() {
    gl_FragColor = textureCube(uSampler, vUv.xyz);
}
