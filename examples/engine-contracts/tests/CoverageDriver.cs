using Godot;

// Test-only process owner. Production _Ready and ordinary Quit have returned first.
public partial class CoverageDriver : Node
{
    public override void _Ready()
    {
        var exitCode = 3;
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
            exitCode = harbour.ExitCode;
        }
        catch (System.Exception error)
        {
            System.Console.Error.WriteLine($"Coverage harness failed: {error.Message}");
        }
        try
        {
            if (OS.GetEnvironment("HARBOUR_COVERAGE") == "1")
                FlushCoverage();
        }
        catch (System.Exception error)
        {
            System.Console.Error.WriteLine($"Coverage flush failed: {error.Message}");
            exitCode = 3;
        }
        GetTree().Quit(exitCode);
    }

    private static void FlushCoverage()
    {
        // Coverlet 10.1.0's injected unload callback flushes hits without ending the CLR.
        // Environment.Exit also runs Godot's native shutdown from the CLR shutdown thread.
        foreach (
            var assembly in new[]
            {
                typeof(HarbourNode).Assembly,
                typeof(Harbour.Consumer).Assembly,
            }
        )
        {
            var tracker = assembly
                .GetTypes()
                .Single(type => type.Namespace == "Coverlet.Core.Instrumentation.Tracker");
            var unload =
                tracker.GetMethod(
                    "UnloadModule",
                    new[] { typeof(object), typeof(System.EventArgs) }
                ) ?? throw new System.InvalidOperationException("Missing Coverlet unload callback");
            unload.CreateDelegate<System.EventHandler>()(null, System.EventArgs.Empty);
        }
    }
}
