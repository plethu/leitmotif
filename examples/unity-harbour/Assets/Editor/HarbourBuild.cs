using System;
using System.IO;
using UnityEditor;
using UnityEditor.Build;
using UnityEditor.Build.Reporting;
using UnityEditor.PackageManager;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class HarbourBuild
{
    public static void CheckEditor()
    {
        // The runtime must come from the installed package, never from source references.
        var package = PackageInfo.FindForAssembly(typeof(HarbourRunner).Assembly);
        var expected = Argument("-harbourPackageVersion");
        if (
            package == null
            || package.name != "com.plethu.leitmotif.harbour"
            || package.version != expected
        )
            throw new InvalidOperationException(
                $"Expected installed package {expected}, found {package?.name}@{package?.version}"
            );
        Debug.Log($"Unity harbour package: {package.name}@{package.version} ({package.source})");
        var host = new GameObject("Harbour");
        try
        {
            // Editor check initializes explicitly; the player exercises Unity's ordinary Awake.
            var runner = host.AddComponent<HarbourRunner>();
            runner.SendMessage("Awake");
            HarbourAcceptance.Check(runner);
        }
        finally
        {
            UnityEngine.Object.DestroyImmediate(host);
        }
    }

    private static string Argument(string name)
    {
        var args = Environment.GetCommandLineArgs();
        var index = Array.IndexOf(args, name);
        if (index < 0 || index + 1 >= args.Length)
            throw new ArgumentException("Missing Unity build argument: " + name);
        return args[index + 1];
    }

    public static void Build()
    {
        var backendName = Argument("-harbourBackend");
        if (
            !Enum.TryParse(backendName, out ScriptingImplementation backend)
            || !Enum.IsDefined(typeof(ScriptingImplementation), backend)
        )
            throw new NotSupportedException(
                "Unavailable explicit Unity scripting backend: " + backendName
            );
        PlayerSettings.SetScriptingBackend(NamedBuildTarget.Standalone, backend);
        // The CoreCLR technical preview has a separate linker/runtime path.
        var stripping =
            backend.ToString() == "CoreCLR"
                ? ManagedStrippingLevel.Minimal
                : ManagedStrippingLevel.High;
        PlayerSettings.SetManagedStrippingLevel(NamedBuildTarget.Standalone, stripping);
        if (PlayerSettings.GetScriptingBackend(NamedBuildTarget.Standalone) != backend)
            throw new InvalidOperationException("Unity did not select " + backendName);
        var scene = EditorSceneManager.NewScene(NewSceneSetup.EmptyScene);
        var host = new GameObject("Harbour");
        host.AddComponent<HarbourRunner>();
        host.AddComponent<HarbourAcceptance>();
        EditorSceneManager.SaveScene(scene, "Assets/Harbour.unity");
        var path = Argument("-harbourPlayer");
        if (string.IsNullOrEmpty(path) || !Path.IsPathRooted(path))
            throw new InvalidOperationException(
                "An absolute -harbourPlayer output path is required."
            );
        var report = BuildPipeline.BuildPlayer(
            new BuildPlayerOptions
            {
                scenes = new[] { "Assets/Harbour.unity" },
                locationPathName = path,
                target = BuildTarget.StandaloneLinux64,
                options = BuildOptions.None,
            }
        );
        if (report.summary.result != BuildResult.Succeeded)
            throw new InvalidOperationException(
                "Harbour player build failed: " + report.summary.result
            );
        Debug.Log($"Unity harbour build passed: {backendName}; stripping: {stripping}");
    }
}
