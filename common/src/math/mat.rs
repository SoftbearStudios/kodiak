use glam::{Mat4, Vec4};

/// Sets scale to (1,1,1) while preserving rotation and translation.
pub fn normalize_scale_mat4(mat: &Mat4) -> Mat4 {
    let mut ret = *mat;
    // The upper left 3x3 part contains the scale (on the diagonal) muliplied by
    // the rotation. Normalizing removes the scale.
    for i in 0..3 {
        let col = mat.col(i);
        let normalized_xyz = col.truncate().normalize();
        *ret.col_mut(i) = Vec4::from((normalized_xyz, col.w));
    }
    ret
}