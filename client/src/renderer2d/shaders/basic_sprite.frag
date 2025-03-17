precision mediump float;

varying highp vec2 vPosition;
varying highp vec2 vUv;
varying float vAlpha;

uniform sampler2D uColor;

void main() {
    gl_FragColor = texture2D(uColor, vUv);
    gl_FragColor *= vAlpha;
}
