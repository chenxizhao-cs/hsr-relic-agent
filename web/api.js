export class DemoApi {
  constructor() {
    this.token = sessionStorage.getItem('hsr-demo-session')
  }
  async request(path, body) {
    const response = await fetch(`/api/${path}`, {
      method: body === undefined ? 'GET' : 'POST',
      headers: { 'content-type': 'application/json', ...(this.token ? { 'x-demo-session': this.token } : {}) },
      ...(body === undefined ? {} : { body: JSON.stringify(body) }),
    })
    const data = await response.json()
    if (!response.ok) {
      const error = new Error(data.error?.message ?? '服务暂时无法处理这次操作。')
      error.code = data.error?.code
      throw error
    }
    return data
  }
  async start() {
    if (this.token) {
      try {
        return await this.request('state')
      } catch (e) {
        if (e.code !== 'session_expired') throw e
      }
    }
    const { session, state } = await this.request('session', {})
    this.token = session
    sessionStorage.setItem('hsr-demo-session', session)
    return state
  }
  action(action, revision) {
    return this.request('action', { ...action, expected_revision: revision })
  }
}
