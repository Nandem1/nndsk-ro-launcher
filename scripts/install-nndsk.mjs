import { spawnSync } from 'node:child_process'
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs'
import { homedir, tmpdir } from 'node:os'
import { basename, dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const tauriConfig = JSON.parse(
  readFileSync(join(projectRoot, 'src-tauri', 'tauri.conf.json'), 'utf8'),
)
const appImageArchitecture = {
  arm64: 'aarch64',
  x64: 'amd64',
}[process.arch]

if (!appImageArchitecture) {
  throw new Error(`Arquitectura no soportada para AppImage: ${process.arch}`)
}

const { productName, version } = tauriConfig
if (typeof productName !== 'string' || typeof version !== 'string') {
  throw new Error(
    'productName y version deben estar definidos en src-tauri/tauri.conf.json',
  )
}

const artifactPath = join(
  projectRoot,
  'target',
  'release',
  'bundle',
  'appimage',
  `${productName}_${version}_${appImageArchitecture}.AppImage`,
)
if (!existsSync(artifactPath)) {
  throw new Error(`No se encontró el AppImage compilado: ${artifactPath}`)
}

const dataHome = process.env.XDG_DATA_HOME || join(homedir(), '.local', 'share')
const applicationsDirectory = join(dataHome, 'applications')
const removedLegacyFiles = removeLegacyManualIntegration(applicationsDirectory)
let integrations = findAppImageLauncherIntegrations(applicationsDirectory)

if (integrations.length > 1) {
  throw new Error(
    `Hay más de una integración de RO-Launcher en AppImageLauncher:\n${integrations
      .map(({ desktopFile }) => `- ${desktopFile}`)
      .join('\n')}`,
  )
}

let installedAppImage
if (integrations.length === 1) {
  installedAppImage = integrations[0].appImage
  replaceFile(artifactPath, installedAppImage, 0o755)
  run('ail-cli', ['integrate', installedAppImage])
} else {
  installedAppImage = integrateFirstBuild(artifactPath)
}

integrations = findAppImageLauncherIntegrations(applicationsDirectory)
if (integrations.length !== 1) {
  throw new Error(
    `AppImageLauncher no dejó una integración única de RO-Launcher (encontradas: ${integrations.length})`,
  )
}

const integration = integrations[0]
if (integration.appImage !== installedAppImage) {
  installedAppImage = integration.appImage
}
normalizeIntegrationDesktop(integration.desktopFile, productName)
run('update-desktop-database', [applicationsDirectory])

const installedSize = statSync(installedAppImage).size
console.log(`RO-Launcher ${version} actualizado (${installedSize} bytes)`)
console.log(`AppImage: ${installedAppImage}`)
console.log(`Entrada: ${integration.desktopFile}`)
if (removedLegacyFiles.length > 0) {
  console.log(
    'Eliminada la integración manual anterior que causaba el duplicado',
  )
}
console.log('Disponible en Walker con Super + Espacio')

function replaceFile(source, destination, mode) {
  mkdirSync(dirname(destination), { recursive: true })
  const temporary = `${destination}.tmp-${process.pid}`

  try {
    copyFileSync(source, temporary)
    chmodSync(temporary, mode)
    renameSync(temporary, destination)
  } finally {
    rmSync(temporary, { force: true })
  }
}

function replaceTextFile(destination, contents) {
  mkdirSync(dirname(destination), { recursive: true })
  const temporary = `${destination}.tmp-${process.pid}`

  try {
    writeFileSync(temporary, contents, { mode: 0o644 })
    renameSync(temporary, destination)
  } finally {
    rmSync(temporary, { force: true })
  }
}

function removeLegacyManualIntegration(applicationsDirectory) {
  const legacyFiles = [
    join(homedir(), '.local', 'bin', 'ro-launcher.AppImage'),
    join(applicationsDirectory, 'RO-Launcher.desktop'),
    join(applicationsDirectory, 'icons', 'RO-Launcher.png'),
  ]
  const removed = []

  for (const path of legacyFiles) {
    if (!existsSync(path)) {
      continue
    }
    rmSync(path)
    removed.push(path)
  }

  return removed
}

function findAppImageLauncherIntegrations(applicationsDirectory) {
  if (!existsSync(applicationsDirectory)) {
    return []
  }

  return readdirSync(applicationsDirectory)
    .filter((name) => name.endsWith('.desktop'))
    .flatMap((name) => {
      const desktopFile = join(applicationsDirectory, name)
      const contents = readFileSync(desktopFile, 'utf8')
      const belongsToAppImageLauncher = contents.includes(
        'X-AppImageLauncher-Version=',
      )
      const belongsToLauncher =
        /^StartupWMClass=ro-launcher$/m.test(contents) ||
        /^X-AppImage-Old-Icon=ro-launcher$/m.test(contents)
      if (!belongsToAppImageLauncher || !belongsToLauncher) {
        return []
      }

      const appImage =
        desktopValue(contents, 'TryExec') ?? desktopValue(contents, 'Exec')
      if (!appImage || !existsSync(appImage)) {
        return []
      }
      return [{ appImage, desktopFile }]
    })
}

function desktopValue(contents, key) {
  const match = contents.match(new RegExp(`^${key}=(.+)$`, 'm'))
  if (!match) {
    return undefined
  }
  return match[1].replace(/^"(.*)"$/, '$1')
}

function integrateFirstBuild(source) {
  const stagingDirectory = mkdtempSync(join(tmpdir(), 'ro-launcher-appimage-'))
  const stagingAppImage = join(stagingDirectory, basename(source))

  try {
    replaceFile(source, stagingAppImage, 0o755)
    run('ail-cli', ['integrate', stagingAppImage])
    const integration = findAppImageLauncherIntegrations(applicationsDirectory)
    if (integration.length !== 1) {
      throw new Error('AppImageLauncher no registró el primer build')
    }
    return integration[0].appImage
  } finally {
    rmSync(stagingDirectory, { force: true, recursive: true })
  }
}

function normalizeIntegrationDesktop(desktopFile, name) {
  const contents = readFileSync(desktopFile, 'utf8')
  const normalized = contents
    .replace(/^Name=.*$/m, `Name=${name}`)
    .replace(/^Categories=.*$/m, 'Categories=Game;')

  if (normalized !== contents) {
    replaceTextFile(desktopFile, normalized)
  }
  run('desktop-file-validate', [desktopFile])
}

function run(command, args) {
  const result = spawnSync(command, args, { stdio: 'inherit' })

  if (result.error) {
    throw result.error
  }
  if (result.status !== 0) {
    throw new Error(`${command} terminó con código ${result.status}`)
  }
}
