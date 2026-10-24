using Godot;

// Test-only process owner. Production _Ready and ordinary Quit have returned first.
public partial class CoverageDriver : Node
{
    public override void _Ready()
    {
        try
        {
            var mode = OS.GetEnvironment("HARBOUR_TEST_MODE");
            HarbourNode harbour =
                mode == "abort-node" ? new AbortingHarbourNode() : new HarbourNode();
            AddChild(harbour);
            if (!harbour.Completed)
                throw new System.InvalidOperationException("Production Node did not complete");
            if (mode == "fail-after-node")
                throw new System.InvalidOperationException(
                    "intentional post-completion test failure"
                );
            AcceptanceCases.Run(System.IO.Path.GetDirectoryName(OS.GetCmdlineUserArgs()[0])!);
            System.Console.WriteLine("Coverage harness complete");
            System.Environment.Exit(harbour.ExitCode);
        }
        catch (System.Exception error)
        {
            System.Console.Error.WriteLine($"Coverage harness failed: {error.Message}");
            System.Environment.Exit(3);
        }
    }
}
