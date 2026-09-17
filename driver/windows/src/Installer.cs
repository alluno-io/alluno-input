// =============================================================================
// AllunoInput Installer
// =============================================================================
//
// This installer supports two modes:
//
//   1. Secure Boot OFF  -> enables test signing, installs driver normally
//   2. Secure Boot ON   -> requires Custom Kernel Signers (CKS) to be set up first
//
// -----------------------------------------------------------------------------
// Custom Kernel Signers (CKS) Setup Guide
// -----------------------------------------------------------------------------
//
// CKS lets you trust your own kernel drivers with Secure Boot ON, without
// submitting to Microsoft for WHQL/attestation signing.
//
// Requirements:
//   - Windows 11 24H2+ with April 2026 update (KB5079391, build 26200.8116+)
//   - Alluno.cer enrolled in UEFI KEK (not db)
//   - Alluno.pfx for signing (password: keep it safe, losing it = unrecoverable)
//   - Microsoft certs for UEFI (so Windows can still boot and update)
//
// Files you need:
//   - Alluno.cer / Alluno.pfx          -> certs/ directory in this repo
//
// Step 1: Add Alluno.cer to UEFI KEK
//   a. Enter BIOS/UEFI setup -> Secure Boot -> Key Management
//   b. Append Alluno.cer to KEK (don't clear existing keys!)
//      Microsoft's certs (UEFI CA 2023, KEK CA 2K 2023) are already present
//      by default on Secure Boot devices - just append, don't replace.
//   c. If your BIOS doesn't support append: you must enter Setup Mode (clears
//      all keys), then re-add Microsoft's certs + Alluno.cer manually.
//      Download Microsoft certs from:
//      https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/windows-secure-boot-key-creation-and-management-guidance
//   d. Save and boot into Windows, install the April 2026 update (KB5079391)
//
// Step 2: Create the App Control policy
//   # Scan device for all existing driver signers (GPU, audio, etc.)
//   New-CIPolicy -ScanPath 'C:\' -UserPEs -NoScript `
//       -FilePath '.\ScannedPolicy.xml' `
//       -Level PCACertificate -Fallback Hash
//
//   # Merge with default Windows policy
//   Merge-CIPolicy `
//       -PolicyPaths 'C:\Windows\schemas\CodeIntegrity\ExamplePolicies\DefaultWindows_Enforced.xml', '.\ScannedPolicy.xml' `
//       -OutputFilePath '.\CustomKernelSignersPolicy.xml'
//
// Step 3: Add Alluno cert to the policy
//   # As policy signer (signs the policy itself)
//   Add-SignerRule -CertificatePath .\Alluno.cer -FilePath .\CustomKernelSignersPolicy.xml -Update
//
//   # As kernel driver signer (trusts our .sys files)
//   Add-SignerRule -CertificatePath .\Alluno.cer -FilePath .\CustomKernelSignersPolicy.xml -Kernel
//
// Step 4: Configure policy options
//   # Policy must be signed (remove unsigned option)
//   Set-RuleOption -Option 6 -FilePath .\CustomKernelSignersPolicy.xml -Delete
//
//   # Optional: kernel-mode only (skip user-mode enforcement)
//   Set-RuleOption -Option 0 -FilePath .\CustomKernelSignersPolicy.xml -Delete
//   # If you remove Option 0, also delete the <SigningScenario Value="12"> element from the XML
//
// Step 5: Convert and sign the policy
//   Set-CIPolicyIdInfo -ResetPolicyID -FilePath .\CustomKernelSignersPolicy.xml
//   $PolicyId = ([xml](Get-Content .\CustomKernelSignersPolicy.xml)).SiPolicy.PolicyId
//   ConvertFrom-CIPolicy .\CustomKernelSignersPolicy.xml -BinaryFilePath ("./" + $PolicyId + ".cip")
//
//   # Sign with Alluno.pfx (OID 1.3.6.1.4.1.311.79.1 is required for CI policy signing)
//   signtool.exe sign /fd sha256 /p7 .\ /p7co 1.3.6.1.4.1.311.79.1 `
//       /p7ce DetachedSignedData /a /f Alluno.pfx /p <password> `
//       ("./" + $PolicyId + ".cip")
//
// Step 6: Deploy to EFI partition
//   mountvol s: /s
//   copy ("./" + $PolicyId + ".cip") s:\EFI\Microsoft\Boot\CiPolicies\Active\
//
// Step 7: Reset Windows (required to activate CKS)
//   Settings > System > Recovery > Reset this PC
//   (This is NOT a full reinstall - it resets Windows while keeping EFI intact)
//
// Step 8: After reset, remove the default Windows kernel policy
//   mountvol s: /s
//   del s:\EFI\Microsoft\Boot\CiPolicies\Active\{8F9CB695-5D48-48D6-A329-7202B44607E3}.cip
//
// Step 9: Run this installer - it will detect CKS and install without test signing
//
// WARNING: Once the signed policy is deployed, it can ONLY be updated with a
// new policy signed by the same Alluno.pfx. If you lose the .pfx, the only
// recovery is disabling Secure Boot in BIOS. Back up your .pfx securely.
//
// Full docs:
//   https://learn.microsoft.com/en-us/windows/security/application-security/application-control/app-control-for-business/design/custom-kernel-signers
//   https://learn.microsoft.com/en-us/windows/security/application-security/application-control/app-control-for-business/operations/citool-commands
//   https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/windows-secure-boot-key-creation-and-management-guidance
// =============================================================================

using System;
using System.Linq;
using System.Reflection;
using Microsoft.Win32;
using System.IO;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Security.Principal;
using System.Text;

namespace AllunoInputInstaller
{
    class Program
    {
        const string KEYBOARD_FILTER_KEY = @"System\CurrentControlSet\Control\Class\{4D36E96B-E325-11CE-BFC1-08002BE10318}";
        const string MOUSE_FILTER_KEY = @"System\CurrentControlSet\Control\Class\{4D36E96F-E325-11CE-BFC1-08002BE10318}";
        const string UpperFilters = "UpperFilters";
        const string KeyboardDriverFileName = "KeyboardAllunoInput.sys";
        const string MouseDriverFileName = "MouseAllunoInput.sys";
        const string KeyboardServiceName = "KeyboardAllunoInput";
        const string MouseServiceName = "MouseAllunoInput";
        const string kbdclass = "kbdclass";
        const string mouclass = "mouclass";

        // AllunoVHID is a root-enumerated bus device, installed from its INF
        // rather than registered as a filter service.
        const string VhidDriverFileName = "AllunoVHID.sys";
        const string VhidInfName = "AllunoVHID.inf";
        const string VhidHardwareId = @"Root\AllunoVHID";
        static readonly Guid SystemClassGuid = new Guid("4d36e97d-e325-11ce-bfc1-08002be10318");

        static void Main(string[] args)
        {
            if (!IsAdmin())
            {
                try
                {
                    ProcessStartInfo psi = new ProcessStartInfo();
                    psi.FileName = Assembly.GetExecutingAssembly().Location;
                    psi.Verb = "runas";
                    if (args.Length > 0)
                        psi.Arguments = string.Join(" ", args);
                    Process.Start(psi);
                }
                catch (Exception) { Console.WriteLine("Administrator rights required."); Pause(); }
                return;
            }

            string action = "install";
            if (args.Length > 0)
                action = args[0].ToLower();
            else
            {
                string exeName = Path.GetFileNameWithoutExtension(Assembly.GetExecutingAssembly().Location).ToLower();
                if (exeName.Contains("uninstall"))
                    action = "uninstall";
            }

            if (action == "uninstall")
                Uninstall();
            else
                Install();

            Pause();
        }

        static void Install()
        {
            Console.WriteLine("AllunoInput - Installing...\n");

            if (IsSecureBootEnabled())
            {
                if (IsCustomKernelSignersActive())
                {
                    Console.WriteLine("  Secure Boot ON + Custom Kernel Signers active.");
                    Console.WriteLine("  Installing without test signing.\n");
                }
                else
                {
                    Console.WriteLine("  ERROR: Secure Boot is enabled but Custom Kernel Signers is not active.");
                    Console.WriteLine("  Installing an unsigned filter driver will brick keyboard/mouse on reboot.");
                    Console.WriteLine("");
                    Console.WriteLine("  Options:");
                    Console.WriteLine("    1. Disable Secure Boot in UEFI/BIOS, then run this installer again");
                    Console.WriteLine("    2. Set up Custom Kernel Signers (Win 11 24H2 + April 2026 update)");
                    Console.WriteLine("");
                    Console.WriteLine("  Installation aborted to protect your system.");
                    return;
                }
            }
            else
            {
                // Enable test signing (required for self-signed kernel drivers)
                RunCmd("bcdedit", "/set testsigning on");
            }

            RemoveLegacyFilters();
            InstallDriver(KeyboardDriverFileName, KeyboardServiceName, KEYBOARD_FILTER_KEY, kbdclass, "keyboard");
            InstallDriver(MouseDriverFileName, MouseServiceName, MOUSE_FILTER_KEY, mouclass, "mouse");
            InstallVhid();

            Console.WriteLine("\nInstallation complete. Please reboot.");
        }

        static void Uninstall()
        {
            Console.WriteLine("AllunoInput - Uninstalling...\n");

            RemoveLegacyFilters();
            UninstallDriver(KeyboardDriverFileName, KeyboardServiceName, KEYBOARD_FILTER_KEY, "keyboard");
            UninstallDriver(MouseDriverFileName, MouseServiceName, MOUSE_FILTER_KEY, "mouse");
            UninstallVhid();

            if (!IsSecureBootEnabled())
                RunCmd("bcdedit", "/set testsigning off");

            Console.WriteLine("\nUninstall complete. Please reboot.");
        }

        const string LegacyKeyboardDriverFileName = "KeyboardAllunoHInpuD.sys";
        const string LegacyMouseDriverFileName = "MouseAllunoHInpuD.sys";
        const string LegacyKeyboardServiceName = "KeyboardAllunoHInpuD";
        const string LegacyMouseServiceName = "MouseAllunoHInpuD";

        static void RemoveLegacyFilters()
        {
            if (!ServiceExists(LegacyKeyboardServiceName) && !ServiceExists(LegacyMouseServiceName))
                return;
            Console.WriteLine("  Removing the AllunoHInpuD filters from an earlier version.");
            UninstallDriver(LegacyKeyboardDriverFileName, LegacyKeyboardServiceName, KEYBOARD_FILTER_KEY, "legacy keyboard");
            UninstallDriver(LegacyMouseDriverFileName, LegacyMouseServiceName, MOUSE_FILTER_KEY, "legacy mouse");
        }

        static bool ServiceExists(string serviceName)
        {
            using (RegistryKey key = Registry.LocalMachine.OpenSubKey(@"System\CurrentControlSet\Services\" + serviceName))
                return key != null;
        }

        static bool InstallDriver(string driverFile, string serviceName, string filterKey, string classService, string label)
        {
            try
            {
                string src = Path.Combine(GetDir(), driverFile);
                if (!File.Exists(src))
                {
                    Console.WriteLine(string.Format("  {0} not found.", driverFile));
                    return false;
                }

                string dest = Path.Combine(Environment.SystemDirectory, "drivers", driverFile);
                File.Copy(src, dest, true);
                Console.WriteLine(string.Format("  Copied {0}", driverFile));

                RunCmd("sc.exe", string.Format("create {0} type= kernel binPath= {1} DisplayName= {0}", serviceName, dest));
                SetRegistryKeyValue(filterKey, classService, serviceName);
                Console.WriteLine(string.Format("  Installed {0} driver.", label));
                return true;
            }
            catch (Exception ex)
            {
                Console.WriteLine(string.Format("  Failed to install {0}: {1}", label, ex.Message));
                return false;
            }
        }

        static void UninstallDriver(string driverFile, string serviceName, string filterKey, string label)
        {
            try
            {
                string dest = Path.Combine(Environment.SystemDirectory, "drivers", driverFile);
                if (File.Exists(dest)) File.Delete(dest);
            }
            catch (Exception) { }

            RunCmd("sc.exe", string.Format("delete {0}", serviceName));
            RemoveRegistryKeyValue(filterKey, serviceName);
            Console.WriteLine(string.Format("  Uninstalled {0} driver.", label));
        }

        // =====================================================================
        // AllunoVHID: the virtual HID bus
        // =====================================================================

        [StructLayout(LayoutKind.Sequential)]
        struct SP_DEVINFO_DATA
        {
            public uint cbSize;
            public Guid ClassGuid;
            public uint DevInst;
            public IntPtr Reserved;
        }

        [DllImport("setupapi.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        static extern IntPtr SetupDiCreateDeviceInfoList(ref Guid ClassGuid, IntPtr hwndParent);

        [DllImport("setupapi.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        static extern bool SetupDiCreateDeviceInfo(IntPtr DeviceInfoSet, string DeviceName, ref Guid ClassGuid,
            string DeviceDescription, IntPtr hwndParent, uint CreationFlags, ref SP_DEVINFO_DATA DeviceInfoData);

        [DllImport("setupapi.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        static extern bool SetupDiSetDeviceRegistryProperty(IntPtr DeviceInfoSet, ref SP_DEVINFO_DATA DeviceInfoData,
            uint Property, byte[] PropertyBuffer, uint PropertyBufferSize);

        [DllImport("setupapi.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        static extern bool SetupDiGetDeviceRegistryProperty(IntPtr DeviceInfoSet, ref SP_DEVINFO_DATA DeviceInfoData,
            uint Property, out uint PropertyRegDataType, byte[] PropertyBuffer, uint PropertyBufferSize, out uint RequiredSize);

        [DllImport("setupapi.dll", SetLastError = true)]
        static extern bool SetupDiCallClassInstaller(uint InstallFunction, IntPtr DeviceInfoSet, ref SP_DEVINFO_DATA DeviceInfoData);

        [DllImport("setupapi.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        static extern IntPtr SetupDiGetClassDevs(IntPtr ClassGuid, string Enumerator, IntPtr hwndParent, uint Flags);

        [DllImport("setupapi.dll", SetLastError = true)]
        static extern bool SetupDiEnumDeviceInfo(IntPtr DeviceInfoSet, uint MemberIndex, ref SP_DEVINFO_DATA DeviceInfoData);

        [DllImport("setupapi.dll", SetLastError = true)]
        static extern bool SetupDiDestroyDeviceInfoList(IntPtr DeviceInfoSet);

        [DllImport("newdev.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        static extern bool UpdateDriverForPlugAndPlayDevices(IntPtr hwndParent, string HardwareId, string FullInfPath,
            uint InstallFlags, out bool bRebootRequired);

        const uint DICD_GENERATE_ID = 0x1;
        const uint SPDRP_HARDWAREID = 0x1;
        const uint DIF_REMOVE = 0x5;
        const uint DIF_REGISTERDEVICE = 0x19;
        const uint DIGCF_ALLCLASSES = 0x4;
        const uint INSTALLFLAG_FORCE = 0x1;

        static void InstallVhid()
        {
            string inf = Path.Combine(GetDir(), VhidInfName);
            string sys = Path.Combine(GetDir(), VhidDriverFileName);
            if (!File.Exists(inf) || !File.Exists(sys))
            {
                Console.WriteLine("  AllunoVHID.inf or AllunoVHID.sys not found; the virtual HID bus was not installed.");
                return;
            }
            try
            {
                if (!VhidDevicePresent() && !CreateVhidDevice())
                {
                    Console.WriteLine(string.Format("  Failed to create the AllunoVHID device node: {0}", Marshal.GetLastWin32Error()));
                    return;
                }
                bool reboot;
                if (!UpdateDriverForPlugAndPlayDevices(IntPtr.Zero, VhidHardwareId, inf, INSTALLFLAG_FORCE, out reboot))
                {
                    Console.WriteLine(string.Format("  Failed to install AllunoVHID.inf: {0}", Marshal.GetLastWin32Error()));
                    return;
                }
                Console.WriteLine("  Installed the AllunoVHID bus.");
            }
            catch (Exception ex)
            {
                Console.WriteLine(string.Format("  Failed to install the AllunoVHID bus: {0}", ex.Message));
            }
        }

        static void UninstallVhid()
        {
            try
            {
                int removed = RemoveVhidDevices();
                if (removed > 0)
                    Console.WriteLine(string.Format("  Removed {0} AllunoVHID device node(s).", removed));
                string oem = FindOemInf("allunovhid.inf");
                if (oem != null)
                    RunCmd("pnputil", string.Format("/delete-driver {0} /uninstall /force", oem));
                Console.WriteLine("  Uninstalled the AllunoVHID bus.");
            }
            catch (Exception ex)
            {
                Console.WriteLine(string.Format("  Failed to uninstall the AllunoVHID bus: {0}", ex.Message));
            }
        }

        static bool CreateVhidDevice()
        {
            Guid classGuid = SystemClassGuid;
            IntPtr set = SetupDiCreateDeviceInfoList(ref classGuid, IntPtr.Zero);
            if (set == IntPtr.Zero || set == new IntPtr(-1))
                return false;
            try
            {
                SP_DEVINFO_DATA data = new SP_DEVINFO_DATA();
                data.cbSize = (uint)Marshal.SizeOf(typeof(SP_DEVINFO_DATA));
                if (!SetupDiCreateDeviceInfo(set, "System", ref classGuid, null, IntPtr.Zero, DICD_GENERATE_ID, ref data))
                    return false;
                byte[] hardwareIds = Encoding.Unicode.GetBytes(VhidHardwareId + "\0\0");
                if (!SetupDiSetDeviceRegistryProperty(set, ref data, SPDRP_HARDWAREID, hardwareIds, (uint)hardwareIds.Length))
                    return false;
                return SetupDiCallClassInstaller(DIF_REGISTERDEVICE, set, ref data);
            }
            finally
            {
                SetupDiDestroyDeviceInfoList(set);
            }
        }

        static bool VhidDevicePresent()
        {
            return ForEachVhidDevice(false) > 0;
        }

        static int RemoveVhidDevices()
        {
            return ForEachVhidDevice(true);
        }

        // Walks every root-enumerated device and counts (or removes) the ones
        // carrying our hardware id.
        static int ForEachVhidDevice(bool remove)
        {
            IntPtr set = SetupDiGetClassDevs(IntPtr.Zero, "ROOT", IntPtr.Zero, DIGCF_ALLCLASSES);
            if (set == IntPtr.Zero || set == new IntPtr(-1))
                return 0;
            int matched = 0;
            try
            {
                SP_DEVINFO_DATA data = new SP_DEVINFO_DATA();
                data.cbSize = (uint)Marshal.SizeOf(typeof(SP_DEVINFO_DATA));
                for (uint index = 0; SetupDiEnumDeviceInfo(set, index, ref data); index++)
                {
                    byte[] buffer = new byte[4096];
                    uint type, required;
                    if (!SetupDiGetDeviceRegistryProperty(set, ref data, SPDRP_HARDWAREID, out type, buffer, (uint)buffer.Length, out required))
                        continue;
                    string ids = Encoding.Unicode.GetString(buffer, 0, (int)Math.Min(required, (uint)buffer.Length));
                    if (ids.IndexOf(VhidHardwareId, StringComparison.OrdinalIgnoreCase) < 0)
                        continue;
                    matched++;
                    if (remove)
                        SetupDiCallClassInstaller(DIF_REMOVE, set, ref data);
                }
            }
            finally
            {
                SetupDiDestroyDeviceInfoList(set);
            }
            return matched;
        }

        // The oemNN.inf name pnputil gave our INF, so the package can be deleted.
        static string FindOemInf(string originalName)
        {
            string output = RunCmdOutput("pnputil", "/enum-drivers");
            if (string.IsNullOrEmpty(output))
                return null;
            string current = null;
            foreach (string raw in output.Split('\n'))
            {
                string line = raw.Trim();
                int colon = line.IndexOf(':');
                if (colon < 0)
                    continue;
                string value = line.Substring(colon + 1).Trim();
                if (value.StartsWith("oem", StringComparison.OrdinalIgnoreCase) && value.EndsWith(".inf", StringComparison.OrdinalIgnoreCase))
                    current = value;
                else if (value.Equals(originalName, StringComparison.OrdinalIgnoreCase) && current != null)
                    return current;
            }
            return null;
        }

        static void SetRegistryKeyValue(string regKey, string classService, string ourServiceName)
        {
            RegistryKey key = Registry.LocalMachine.OpenSubKey(regKey, true);
            string[] current = (string[])key.GetValue(UpperFilters);
            current = current.Where(v => v != ourServiceName).ToArray();
            string[] result = new string[current.Length + 1];
            int idx = 0;
            for (int i = 0; i < current.Length; i++)
            {
                if (current[i] == classService)
                {
                    result[idx] = ourServiceName;
                    result[++idx] = classService;
                }
                else
                    result[idx] = current[i];
                idx++;
            }
            key.SetValue(UpperFilters, result, RegistryValueKind.MultiString);
            key.Close();
        }

        static void RemoveRegistryKeyValue(string regKey, string ourServiceName)
        {
            RegistryKey key = Registry.LocalMachine.OpenSubKey(regKey, true);
            string[] current = (string[])key.GetValue(UpperFilters);
            string[] result = current.Where(v => v != ourServiceName).ToArray();
            key.SetValue(UpperFilters, result, RegistryValueKind.MultiString);
            key.Close();
        }

        static int RunCmd(string exe, string args)
        {
            try
            {
                ProcessStartInfo psi = new ProcessStartInfo();
                psi.FileName = exe;
                psi.Arguments = args;
                psi.WindowStyle = ProcessWindowStyle.Hidden;
                psi.CreateNoWindow = true;
                psi.UseShellExecute = false;
                psi.RedirectStandardOutput = true;
                Process p = Process.Start(psi);
                p.WaitForExit();
                string output = p.StandardOutput.ReadToEnd();
                if (!string.IsNullOrEmpty(output))
                    Console.WriteLine("    " + output.Trim());
                return p.ExitCode;
            }
            catch (Exception ex)
            {
                Console.WriteLine("    " + ex.Message);
                return -1;
            }
        }

        static bool IsSecureBootEnabled()
        {
            try
            {
                RegistryKey key = Registry.LocalMachine.OpenSubKey(@"SYSTEM\CurrentControlSet\Control\SecureBoot\State");
                if (key == null) return false;
                object val = key.GetValue("UEFISecureBootEnabled");
                key.Close();
                return val != null && (int)val == 1;
            }
            catch (Exception) { return false; }
        }

        // Detects if Custom Kernel Signers (CKS) is active on this system.
        //
        // CKS allows organizations to trust kernel drivers signed with their own PKI
        // without WHCP certification. It requires:
        //   - Windows 11 24H2+ or Windows Server 2025+ with April 2026 update (KB5079391)
        //   - Signed App Control policy deployed to EFI System Partition
        //   - UEFI Secure Boot with org-owned PK or KEK
        //   - Push-button reset after policy deployment
        //
        // Setup guide: https://learn.microsoft.com/en-us/windows/security/application-security/application-control/app-control-for-business/design/custom-kernel-signers
        // CiTool reference: https://learn.microsoft.com/en-us/windows/security/application-security/application-control/app-control-for-business/operations/citool-commands
        //
        // Detection: CKS step 9 removes the default Windows kernel policy
        // {8F9CB695-5D48-48D6-A329-7202B44607E3} and steps 4-7 deploy a custom
        // signed policy. We check that the default is gone and policies still exist.
        //
        // NOTE: This check can false-positive before the April 2026 update (KB5079391,
        // builds 26200.8116/26100.8116) since the default kernel policy doesn't exist
        // yet on those builds. The driver still won't load in that case because KMCI
        // requires Microsoft signatures regardless. A future improvement could verify
        // the build number or check for a non-platform signed enforced policy.
        static bool IsCustomKernelSignersActive()
        {
            try
            {
                string output = RunCmdOutput("CiTool", "--list-policies");
                if (string.IsNullOrEmpty(output)) return false;

                bool hasDefaultKernelPolicy = output.IndexOf(
                    "8F9CB695-5D48-48D6-A329-7202B44607E3",
                    StringComparison.OrdinalIgnoreCase) >= 0;
                bool hasPolicies = output.IndexOf("Policy ID", StringComparison.OrdinalIgnoreCase) >= 0;

                return hasPolicies && !hasDefaultKernelPolicy;
            }
            catch (Exception) { return false; }
        }

        static string RunCmdOutput(string exe, string args)
        {
            try
            {
                ProcessStartInfo psi = new ProcessStartInfo();
                psi.FileName = exe;
                psi.Arguments = args;
                psi.WindowStyle = ProcessWindowStyle.Hidden;
                psi.CreateNoWindow = true;
                psi.UseShellExecute = false;
                psi.RedirectStandardOutput = true;
                Process p = Process.Start(psi);
                string output = p.StandardOutput.ReadToEnd();
                p.WaitForExit();
                return output;
            }
            catch (Exception) { return ""; }
        }

        static bool IsAdmin()
        {
            WindowsIdentity identity = WindowsIdentity.GetCurrent();
            WindowsPrincipal principal = new WindowsPrincipal(identity);
            return principal.IsInRole(WindowsBuiltInRole.Administrator);
        }

        static void Pause()
        {
            Console.WriteLine("\nPress any key to exit...");
            try { Console.ReadKey(); } catch (Exception) { }
        }

        static string GetDir()
        {
            return AppDomain.CurrentDomain.BaseDirectory;
        }
    }
}
