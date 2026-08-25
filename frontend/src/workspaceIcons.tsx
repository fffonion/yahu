import {
  Binary,
  BookOpen,
  Braces,
  Code2,
  Database,
  FileArchive,
  FileAudio,
  FileCode2,
  FileCog,
  FileDiff,
  FileImage,
  FileJson,
  FileKey2,
  FileLock2,
  FileSpreadsheet,
  FileTerminal,
  FileText,
  FileType2,
  FileVideo,
  Folder,
  FolderArchive,
  FolderCheck,
  FolderCode,
  FolderCog,
  FolderGit2,
  FolderKanban,
  FolderOpen,
  FolderRoot,
  FolderSync,
  FolderTree,
  Package,
  Palette,
  Settings2,
  TerminalSquare,
  type LucideIcon,
} from 'lucide-react';

type WorkspaceIconKind = 'file' | 'dir';

type WorkspaceIconEntry = {
  name: string;
  kind: WorkspaceIconKind;
};

type WorkspaceIconName =
  | 'binary'
  | 'book-open'
  | 'braces'
  | 'code'
  | 'database'
  | 'file-archive'
  | 'file-audio'
  | 'file-code'
  | 'file-cog'
  | 'file-diff'
  | 'file-image'
  | 'file-json'
  | 'file-key'
  | 'file-lock'
  | 'file-spreadsheet'
  | 'file-terminal'
  | 'file-text'
  | 'file-type'
  | 'file-video'
  | 'folder'
  | 'folder-archive'
  | 'folder-check'
  | 'folder-code'
  | 'folder-cog'
  | 'folder-git'
  | 'folder-kanban'
  | 'folder-open'
  | 'folder-root'
  | 'folder-sync'
  | 'folder-tree'
  | 'package'
  | 'palette'
  | 'settings'
  | 'terminal';

type WorkspaceIconTone =
  | 'default'
  | 'typescript'
  | 'javascript'
  | 'python'
  | 'rust'
  | 'go'
  | 'java'
  | 'c-family'
  | 'web'
  | 'style'
  | 'data'
  | 'config'
  | 'docs'
  | 'shell'
  | 'package'
  | 'dependency'
  | 'media'
  | 'archive'
  | 'security'
  | 'git'
  | 'folder-code'
  | 'folder-tests'
  | 'folder-docs'
  | 'folder-assets'
  | 'folder-config'
  | 'folder-dependency'
  | 'folder-build'
  | 'folder-data'
  | 'folder-tools';

export type WorkspaceIconDescriptor = {
  icon: WorkspaceIconName;
  tone: WorkspaceIconTone;
};

const iconComponents: Record<WorkspaceIconName, LucideIcon> = {
  binary: Binary,
  'book-open': BookOpen,
  braces: Braces,
  code: Code2,
  database: Database,
  'file-archive': FileArchive,
  'file-audio': FileAudio,
  'file-code': FileCode2,
  'file-cog': FileCog,
  'file-diff': FileDiff,
  'file-image': FileImage,
  'file-json': FileJson,
  'file-key': FileKey2,
  'file-lock': FileLock2,
  'file-spreadsheet': FileSpreadsheet,
  'file-terminal': FileTerminal,
  'file-text': FileText,
  'file-type': FileType2,
  'file-video': FileVideo,
  folder: Folder,
  'folder-archive': FolderArchive,
  'folder-check': FolderCheck,
  'folder-code': FolderCode,
  'folder-cog': FolderCog,
  'folder-git': FolderGit2,
  'folder-kanban': FolderKanban,
  'folder-open': FolderOpen,
  'folder-root': FolderRoot,
  'folder-sync': FolderSync,
  'folder-tree': FolderTree,
  package: Package,
  palette: Palette,
  settings: Settings2,
  terminal: TerminalSquare,
};

const basename = (name: string) => name.trim().toLowerCase().split('/').pop() || '';
const extension = (name: string) => {
  const value = basename(name);
  const dot = value.lastIndexOf('.');
  return dot > 0 ? value.slice(dot) : '';
};

const folderGroups: Array<{ names: Set<string>; icon: WorkspaceIconName; tone: WorkspaceIconTone }> = [
  { names: new Set(['.git']), icon: 'folder-git', tone: 'git' },
  { names: new Set(['src', 'source', 'lib', 'app', 'apps', 'components', 'pages', 'routes', 'modules']), icon: 'folder-code', tone: 'folder-code' },
  { names: new Set(['test', 'tests', '__tests__', 'spec', 'specs', 'fixtures', 'mocks']), icon: 'folder-check', tone: 'folder-tests' },
  { names: new Set(['docs', 'doc', 'documentation', 'wiki']), icon: 'folder-kanban', tone: 'folder-docs' },
  { names: new Set(['assets', 'asset', 'public', 'static', 'images', 'img', 'media', 'fonts']), icon: 'folder-tree', tone: 'folder-assets' },
  { names: new Set(['config', 'configs', '.config', 'settings', '.vscode', '.idea']), icon: 'folder-cog', tone: 'folder-config' },
  { names: new Set(['node_modules', 'vendor', 'deps', 'dependencies', 'third_party']), icon: 'folder-archive', tone: 'folder-dependency' },
  { names: new Set(['target', 'dist', 'build', 'out', 'coverage', '.cache', '.next', '.vite']), icon: 'folder-sync', tone: 'folder-build' },
  { names: new Set(['db', 'database', 'databases', 'migrations', 'schema']), icon: 'folder-root', tone: 'folder-data' },
  { names: new Set(['scripts', 'script', 'bin', 'tools', 'tool', 'commands']), icon: 'folder-code', tone: 'folder-tools' },
];

const specialFiles: Record<string, WorkspaceIconDescriptor> = {
  'package.json': { icon: 'package', tone: 'package' },
  'package-lock.json': { icon: 'package', tone: 'dependency' },
  'pnpm-lock.yaml': { icon: 'package', tone: 'dependency' },
  'yarn.lock': { icon: 'package', tone: 'dependency' },
  'bun.lock': { icon: 'package', tone: 'dependency' },
  'bun.lockb': { icon: 'package', tone: 'dependency' },
  'cargo.toml': { icon: 'package', tone: 'rust' },
  'cargo.lock': { icon: 'package', tone: 'rust' },
  makefile: { icon: 'file-terminal', tone: 'shell' },
  dockerfile: { icon: 'file-terminal', tone: 'shell' },
  'docker-compose.yml': { icon: 'file-terminal', tone: 'shell' },
  'docker-compose.yaml': { icon: 'file-terminal', tone: 'shell' },
  '.gitignore': { icon: 'file-cog', tone: 'git' },
  '.dockerignore': { icon: 'file-cog', tone: 'config' },
  '.editorconfig': { icon: 'settings', tone: 'config' },
  '.env': { icon: 'file-key', tone: 'security' },
  '.env.local': { icon: 'file-key', tone: 'security' },
  '.env.production': { icon: 'file-key', tone: 'security' },
  readme: { icon: 'book-open', tone: 'docs' },
  'readme.md': { icon: 'book-open', tone: 'docs' },
  license: { icon: 'book-open', tone: 'docs' },
  'license.md': { icon: 'book-open', tone: 'docs' },
  changelog: { icon: 'file-diff', tone: 'docs' },
  'changelog.md': { icon: 'file-diff', tone: 'docs' },
};

const codeTones: Record<string, WorkspaceIconTone> = {
  '.ts': 'typescript',
  '.tsx': 'typescript',
  '.mts': 'typescript',
  '.cts': 'typescript',
  '.js': 'javascript',
  '.jsx': 'javascript',
  '.mjs': 'javascript',
  '.cjs': 'javascript',
  '.py': 'python',
  '.pyw': 'python',
  '.rs': 'rust',
  '.go': 'go',
  '.java': 'java',
  '.kt': 'java',
  '.kts': 'java',
  '.c': 'c-family',
  '.h': 'c-family',
  '.cc': 'c-family',
  '.cpp': 'c-family',
  '.cxx': 'c-family',
  '.hpp': 'c-family',
  '.cs': 'c-family',
  '.swift': 'c-family',
  '.php': 'c-family',
  '.rb': 'c-family',
  '.lua': 'c-family',
  '.zig': 'c-family',
  '.ex': 'c-family',
  '.exs': 'c-family',
};

export function workspaceIconFor(entry: WorkspaceIconEntry, expanded = false): WorkspaceIconDescriptor {
  const name = basename(entry.name);
  if (entry.kind === 'dir') {
    const group = folderGroups.find((item) => item.names.has(name));
    if (group) return { icon: expanded && group.icon === 'folder' ? 'folder-open' : group.icon, tone: group.tone };
    return { icon: expanded ? 'folder-open' : 'folder', tone: 'default' };
  }

  const special = specialFiles[name];
  if (special) return special;

  const ext = extension(name);
  const codeTone = codeTones[ext];
  if (codeTone) return { icon: 'file-code', tone: codeTone };
  if (['.html', '.htm', '.xml', '.xhtml', '.svg', '.vue', '.svelte'].includes(ext)) return { icon: 'file-type', tone: 'web' };
  if (['.css', '.scss', '.sass', '.less', '.styl'].includes(ext)) return { icon: 'palette', tone: 'style' };
  if (['.json', '.jsonc', '.ndjson'].includes(ext)) return { icon: 'file-json', tone: 'data' };
  if (['.yaml', '.yml', '.toml', '.ini', '.conf', '.config', '.properties'].includes(ext)) return { icon: 'file-cog', tone: 'config' };
  if (['.md', '.mdx', '.rst', '.adoc', '.txt', '.log'].includes(ext)) return { icon: 'book-open', tone: 'docs' };
  if (['.sh', '.bash', '.zsh', '.fish', '.ps1', '.bat', '.cmd'].includes(ext)) return { icon: 'file-terminal', tone: 'shell' };
  if (['.sql', '.db', '.sqlite', '.sqlite3', '.mdb'].includes(ext)) return { icon: 'database', tone: 'data' };
  if (['.csv', '.tsv', '.xls', '.xlsx', '.ods'].includes(ext)) return { icon: 'file-spreadsheet', tone: 'data' };
  if (['.png', '.jpg', '.jpeg', '.gif', '.webp', '.avif', '.ico', '.bmp', '.tiff'].includes(ext)) return { icon: 'file-image', tone: 'media' };
  if (['.mp3', '.wav', '.ogg', '.flac', '.m4a', '.aac'].includes(ext)) return { icon: 'file-audio', tone: 'media' };
  if (['.mp4', '.webm', '.mov', '.mkv', '.avi'].includes(ext)) return { icon: 'file-video', tone: 'media' };
  if (['.zip', '.tar', '.gz', '.bz2', '.xz', '.7z', '.rar', '.tgz'].includes(ext)) return { icon: 'file-archive', tone: 'archive' };
  if (['.pem', '.key', '.crt', '.cer', '.p12', '.pfx'].includes(ext)) return { icon: 'file-lock', tone: 'security' };
  if (['.lock'].includes(ext)) return { icon: 'file-lock', tone: 'dependency' };
  if (['.diff', '.patch'].includes(ext)) return { icon: 'file-diff', tone: 'default' };
  if (['.wasm', '.bin', '.dat', '.class', '.o', '.a', '.so', '.dll', '.dylib'].includes(ext)) return { icon: 'binary', tone: 'default' };
  if (['.env', '.npmrc', '.prettierrc', '.eslintrc'].includes(ext)) return { icon: 'file-cog', tone: 'config' };
  if (['.graphql', '.gql', '.proto'].includes(ext)) return { icon: 'braces', tone: 'data' };
  return { icon: 'file-text', tone: 'default' };
}

export function WorkspaceEntryIcon({ entry, expanded = false }: { entry: WorkspaceIconEntry; expanded?: boolean }) {
  const descriptor = workspaceIconFor(entry, expanded);
  const Icon = iconComponents[descriptor.icon];
  return <span className={`workspace-entry-icon icon-tone-${descriptor.tone}`} data-icon={descriptor.icon} aria-hidden="true"><Icon /></span>;
}
