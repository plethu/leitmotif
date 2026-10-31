using System;
using System.IO;
using UnityEngine;

// Test scene only: writes adversarial fixtures in a disposable writable directory.
public sealed class HarbourAcceptance : MonoBehaviour
{
    public static void Check(HarbourRunner runner)
    {
        var directory = Path.Combine(Application.temporaryCachePath, Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(directory);
        try
        {
            foreach (var source in Directory.GetFiles(Application.streamingAssetsPath, "*.json"))
                File.Copy(source, Path.Combine(directory, Path.GetFileName(source)));
            AcceptanceCases.Run(directory, runner.Replay);
            Debug.Log(
                "Unity harbour acceptance passed: four outcomes; 62 adversarial cases; 85 audit actions per report."
            );
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    private void Start()
    {
        try
        {
            Check(GetComponent<HarbourRunner>());
            Application.Quit(0);
        }
        catch (Exception error)
        {
            Debug.LogException(error);
            Application.Quit(3);
        }
    }
}
