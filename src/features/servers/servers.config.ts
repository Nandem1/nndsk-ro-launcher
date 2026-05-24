export interface ServerConfig {
  id: string
  name: string
  executablePath: string
  patcherPath?: string
  winePrefix?: string
  runner?: string // path to wine/proton binary; undefined = use global setting
}
