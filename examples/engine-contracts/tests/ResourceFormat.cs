using Godot;

public partial class ResourceFormat : Node
{
    public override void _Ready()
    {
        foreach (var name in new[] { "Harbour", "Coverage", "Format" })
        {
            var resource = ResourceLoader.Load<PackedScene>($"res://{name}.tscn");
            var path = $"user://{name}.canonical.tscn";
            if (ResourceSaver.Save(resource, path) != Error.Ok)
                throw new System.InvalidOperationException("cannot serialize scene");
            var actual = System.IO.File.ReadAllText(ProjectSettings.GlobalizePath(path));
            var source = ProjectSettings.GlobalizePath($"res://{name}.tscn");
            if (OS.GetCmdlineUserArgs().Contains("--write"))
                System.IO.File.WriteAllText(source, actual);
            else if (System.IO.File.ReadAllText(source) != actual)
                throw new System.InvalidOperationException(
                    $"{name}.tscn needs native Godot serialization"
                );
        }
        var config = new ConfigFile();
        if (
            config.Load("res://project.godot") != Error.Ok
            || config.Save("user://project.canonical.godot") != Error.Ok
        )
            throw new System.InvalidOperationException("cannot round-trip project settings");
        var settings = System.IO.File.ReadAllText(
            ProjectSettings.GlobalizePath("user://project.canonical.godot")
        );
        var project = ProjectSettings.GlobalizePath("res://project.godot");
        if (OS.GetCmdlineUserArgs().Contains("--write"))
            System.IO.File.WriteAllText(project, settings);
        else if (System.IO.File.ReadAllText(project) != settings)
            throw new System.InvalidOperationException(
                "project.godot needs native ConfigFile formatting"
            );
        GetTree().Quit();
    }
}
