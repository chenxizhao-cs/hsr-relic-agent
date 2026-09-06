// The only UI module aware of the resource manifest. No upstream paths in components.
export class AssetProvider {
  constructor(manifest) {
    this.manifest = manifest
  }
  static async load() {
    const response = await fetch('/.generated/assets.json')
    if (!response.ok) throw new Error('图片资源尚未准备好，请运行 Web 准备脚本。')
    return new AssetProvider(await response.json())
  }
  character(id, variant = 'avatar') {
    return this.manifest.characters[id]?.[variant] ?? this.manifest.fallback
  }
  relic(relic) {
    return this.manifest.relics[relic.set_id]?.[relic.slot] ?? this.manifest.fallback
  }
  stat(stat) {
    return this.manifest.stats[stat] ?? this.manifest.fallback
  }
  fallback() {
    return this.manifest.fallback
  }
}
