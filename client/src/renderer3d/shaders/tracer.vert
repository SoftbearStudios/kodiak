attribute vec3 position;
attribute mat4 model;
attribute vec4 color;
uniform mat4 uViewProjection;
varying vec4 vColor;

void main() {
    vColor = color;
    gl_Position = uViewProjection * model * vec4(position, 1.0);
}
