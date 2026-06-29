#define AppName "Git Agent"
#ifndef AppVersion
#define AppVersion "dev"
#endif
#ifndef SourceDir
#define SourceDir "..\\..\\target\\release"
#endif
#ifndef OutputDir
#define OutputDir "..\\..\\dist"
#endif

[Setup]
AppId={{7F2D2B68-AB4B-4B9A-9E91-F3574BA53C5D}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=adoin
DefaultDirName={localappdata}\Programs\GitAgent
DisableDirPage=no
DisableProgramGroupPage=no
DefaultGroupName=Git Agent
PrivilegesRequired=lowest
OutputDir={#OutputDir}
OutputBaseFilename=GitAgentSetup-{#AppVersion}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\git-agent.exe

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Dirs]
Name: "{app}\data"

[Files]
Source: "{#SourceDir}\git-agent.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\git-agent-merge.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\git-agent-diff.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Git Agent"; Filename: "{app}\git-agent.exe"; WorkingDir: "{app}"
Name: "{group}\Uninstall Git Agent"; Filename: "{uninstallexe}"

[Run]
Filename: "{app}\git-agent.exe"; Description: "Launch Git Agent"; Flags: nowait postinstall skipifsilent
