# FreeCAD

## Export to FreeCAD

*File › FreeCAD (.FCStd)* writes a FreeCAD document with every element as a solid, with its BIM type and Volumen id. Open it in FreeCAD, edit it, save it, and link it back.

## Link a FreeCAD file

*File › Import and link…* and pick the `.FCStd` file. Its objects come in and stay linked: with *Reload linked files when saved* ticked, saving in FreeCAD updates them here. *Assign as…* in *Properties* turns an object into a wall, slab, door or window.

## Material cards

*Materials › Import materials…* reads FreeCAD material cards (`.FCMat`), in the old and the FreeCAD 1.x formats. Opening or dropping a `.FCMat` file also adds its material to the project.

You can also exchange with FreeCAD through [IFC](ifc), which its BIM workbench reads and writes.
