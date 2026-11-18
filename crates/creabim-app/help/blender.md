# Blender

Volumen works with Blender both ways: model freely in Blender and bring it in, or send the Volumen model to Blender for rendering.

## From Blender: link a .blend file

1. *File › Import and link…* and pick the `.blend` file.
2. Its objects come in, with their Blender materials.

To have objects come in as real BIM elements:

- put them in a collection named *Walls*, *Slabs*, *Doors* or *Windows*;
- or give an object a `creabim_kind` property, or an `ifc_class` property such as `IfcColumn`;
- or, in Volumen, select the object and use *Assign as…* in *Properties* (for example *Assign as… › Slab*). It gets slab handles, a thickness and a material, and stays linked.

With *File › Reload linked files when saved* ticked, saving in Blender updates Volumen within a second. Walls stay walls and slabs stay slabs.

## To Blender: glTF

*File › glTF (.glb) for Blender* writes every element as a mesh with its Volumen id and materials. In Blender, use *File › Import › glTF*, or the **CreaBIM Link** add-on (in `tools/blender-addon` in the source), which also sends your Blender edits back to the same elements. See [glTF](gltf).

## Materials

*Materials › Import materials…* reads a `.blend` file's materials (the Principled BSDF's colour, roughness, metallic and alpha).
