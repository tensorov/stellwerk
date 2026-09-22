/**
 * Durable fix: OMO hardcodes prometheus bash/interactive_bash = "deny",
 * which strips bash from tools[] and fails the Zen free-tier gate
 * (requires bash, edit, glob, grep, read).
 *
 * Runs after oh-my-openagent (project plugin dir is last in load order).
 * Survives oh-my-openagent@latest cache updates.
 */
const PROMETHEUS_NAME_RE = /prometheus|planner-sisyphus|omo-plan/i

function fixPermission(permission) {
  if (permission === null || typeof permission !== "object") return
  if (permission.bash === "deny") permission.bash = "ask"
  if (permission.interactive_bash === "deny") permission.interactive_bash = "ask"
  if (permission.bash !== null && typeof permission.bash === "object") {
    if (permission.bash["*"] === "deny") permission.bash["*"] = "ask"
  }
}

export const FixPrometheusBash = async () => ({
  config: async (config) => {
    const agents = config?.agent
    if (agents === null || typeof agents !== "object") return
    for (const [name, agent] of Object.entries(agents)) {
      if (!PROMETHEUS_NAME_RE.test(name)) continue
      if (agent === null || typeof agent !== "object") continue
      fixPermission(agent.permission)
    }
  },
})

export default FixPrometheusBash
