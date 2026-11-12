//! Geometry for CreaBIM: turns model elements into triangle meshes for the
//! viewport and into 2D linework for plan drawings.
//!
//! This crate is the seam where a full B-rep kernel (OpenCascade) plugs in
//! later. Architectural elements in the prototype are boxes, extrusions and
//! polygon meshes, which this crate builds exactly.

pub mod annotate;
pub mod annotation;
pub mod crop;
pub mod cropbox;
pub mod curtain;
pub mod declutter;
pub mod detail;
pub mod dxf;
pub mod looks;
pub mod markers;
pub mod members;
pub mod mesh;
pub mod objects;
pub mod opening;
pub mod paper;
pub mod parts;
pub mod pick;
pub mod plan;
pub mod prio;
pub mod ranges;
pub mod raster;
pub mod rooms;
pub mod scene;
pub mod schedule;
pub mod section;
pub mod sheet;
pub mod slice;
pub mod triangulate;
pub mod wall;
pub mod xlsx;

pub use mesh::TriMesh;
pub use plan::{plan_view, PlanItem, PlanView};
pub use scene::{build_scene, shared_scene, shared_scene_ready, SceneItem};
