# Opening and saving

## The project file

A Volumen project is a `.creabim` file. It holds the model, its views, sheets, styles and templates, and the **undo history**: you can still undo after closing and reopening the project.

- `Ctrl+S` or *File › Save* saves. *Save as…* saves under another name.
- `Ctrl+O` or *File › Open…* opens a `.creabim`, `.ifc` or `.ifcx` file.
- *File › Open recent* lists the last ten projects.
- Dropping a file on the window opens it.

Exports are written next to the project file, with its name.

While the project has unsaved changes, a dot shows beside its name in the top bar, and the window's title starts with a •. Closing Volumen then asks first: **Save**, **Don't save** or **Cancel**.

## The activity log

Volumen keeps a plain text log of what you did in this session: each drag frame by frame, every edit and undo, every tool change, and what went wrong. Nothing leaves your computer.

To read it, pick **Log** in an area's view-type menu. The Log view shows the log as it is written, with problems in red. **Problems only** keeps just crashes, errors and what could not be done; **Copy** copies what is shown, to paste in a message; **Open the folder** shows where the whole log is kept.

## Report a problem

When something goes wrong, *Help › Report a problem…* (or the button in the Log view) saves **one file** to attach to a bug report or an email: a summary of the version, your system and what is open, the whole activity log and your settings, and the project as it is now only if you say yes. Nothing is sent anywhere by Volumen: you choose where the file goes.

## Linked files

*File › Import and link…* brings in the objects of a Blender (`.blend`) or FreeCAD (`.FCStd`) file. They stay linked: with *Reload linked files when saved* ticked, saving the file in Blender or FreeCAD updates them here within a second. *Reload a linked file* does it by hand. See [Blender](blender) and [FreeCAD](freecad).

## Exchange formats

| Format | Read | Write | Page |
| --- | --- | --- | --- |
| IFC2X3, IFC4, IFC4X3 (`.ifc`) | yes | IFC4 | [IFC](ifc) |
| IFC5 alpha (`.ifcx`) | yes | yes | [IFC](ifc) |
| Blender (`.blend`) | linked | via glTF | [Blender](blender) |
| FreeCAD (`.FCStd`) | linked | yes | [FreeCAD](freecad) |
| glTF (`.glb`, `.gltf`) | materials | yes | [glTF](gltf) |
| PDF | no | sheets | [PDF, SVG and DXF](dxf-svg) |
| SVG | title blocks | views and sheets | [PDF, SVG and DXF](dxf-svg) |
| DXF | title blocks | no | [PDF, SVG and DXF](dxf-svg) |
| CSV | no | schedules | [Schedules](schedules) |
