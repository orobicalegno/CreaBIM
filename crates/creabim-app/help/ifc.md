# IFC

**IFC** (Industry Foundation Classes) is the open file format that BIM programs use to share models. An IFC file says not just what shapes there are, but what they are: this is a wall on the ground floor, made of brick, with these doors in it. Volumen uses IFC ids for its own elements, so a model that goes out and comes back keeps its identity.

## Exporting

- *File › IFC4 (.ifc)*: the usual IFC file, for Revit, ArchiCAD, Vectorworks, Bonsai (BlenderBIM), FreeCAD, Solibri and IFC viewers.
- *File › IFC5 (.ifcx, alpha)*: the new JSON-based IFC. Its schema is still an alpha, so other tools may not read it yet.

What goes out: levels (storeys), walls with their location line and layers, slabs, roofs, doors and windows with their hand and swing, openings and shafts, columns, beams, stairs, railings, curtain walls with their mullions and panels, types, materials with their colours, furniture and fixtures with their types, and masses with their IFC class. Guides, sketches and crop boxes stay behind.

### How walls, slabs and roofs go out

Before writing, the export asks how walls, slabs and roofs go out, and says what the other program will see under each. The choice is remembered.

| Choice | The other program gets |
| --- | --- |
| **As modelled elements** (recommended) | One IfcWall or IfcSlab each, with its layer set and its plain body. It comes back into Volumen as it went out. |
| **Joined shapes, as drawn** | Their bodies cut where walls run through slabs, as Volumen shows them |
| **Layers as separate elements** | The structure stays the IfcWall or IfcSlab; finishes, insulation and membranes become IfcCovering elements (cladding, flooring, roofing) covering it. For trades and quantities by layer. |

The project's name, site and building, from *Manage › Project information*, name the IFC project, site and building.

## Opening an IFC file

*File › Open…* reads IFC2X3, IFC4 and IFC4X3 files, and `.ifcx`.

- **Walls** become native, editable walls when their body is a straight upright prism, whether the file writes it as an extrusion or as a faceted B-rep or face set (as FreeCAD and many other tools do). A B-rep wall may already have its door and window holes cut: they are read back as openings. A wall drawn round a room along a polyline axis, perhaps one extrusion per layer, becomes one wall per straight run, meeting at the corners.
- **Slabs** become native when they are flat upright prisms, with any holes, and with their top at any height: off a storey, the slab sits on the storey below with a top offset.
- **Doors and windows** become native in a native wall: through the file's opening relationships (`IfcRelVoidsElement`, `IfcRelFillsElement`), or, when the file has none, in the wall their body sits in. They take the size of the hole cut in the wall when there is one, so they show in the door and window schedules and get marks. A hole nothing fills becomes a wall opening.
- **Columns and beams** become native when they are straight prisms: an upright one with a rectangular or round section, a level bar with a rectangular section. Their types come back. Columns, beams, roofs, stairs and railings Volumen wrote come back exactly as they were.
- Types, assemblies, materials and grid systems come back too. Furniture and fixtures with an IfcFurnitureType, IfcSanitaryTerminalType, IfcElectricApplianceType or IfcSystemFurnitureElementType that has a body map come in as [objects](furniture) of that type.
- Everything else with a shape (a gable wall, a sloped roof slab, stairs and railings from other tools, terrain…) comes in as a mesh that keeps its IFC class. Openings in a mesh are cut into it, and its corners are joined up, so it is closed.
- Units, placements and IFC ids are kept, and a plan is made for each storey.

## The import report

When an import has anything to say, the **Import report** opens: how many elements of each IFC class came in native and how many as meshes, the notes grouped by kind with a count, and below them every element that could not come in natively, by class, name and GlobalId, with the reason, for example:

> IfcWall 'Wall-Gable-E' (0NBfRAfdjALxRLSMXhQcSA) is an IfcFacetedBrep that has 2 sloped faces (a raked or gable top, or a battered face); imported as a mesh

*Copy report* copies it all as text, to fix the model in the program that wrote it or to send with a question. *File › Import report* opens it again. `creabim import-ifc` prints the same notes and list.

## Materials only

*Materials › Import materials…* reads just the materials of an IFC file, with their categories and surface styles.
