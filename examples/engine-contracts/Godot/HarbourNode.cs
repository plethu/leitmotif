using Godot;
using Harbour;

public partial class HarbourNode : Node
{
    private readonly Consumer consumer = new();

    public int ExitCode { get; private set; } = 3;
    public bool Completed { get; private set; }

    public override void _Ready()
    {
        ExitCode = consumer.Run(OS.GetCmdlineUserArgs());
        GetTree().Quit(ExitCode);
        Completed = true;
    }
}
