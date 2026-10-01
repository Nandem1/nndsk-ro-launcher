export function qualityCiTriggerError(yaml) {
  const pushBare = /^ {2}push:\s*$/m.test(yaml)
  const restricted = /tags-ignore|branches:/.test(yaml)
  if (pushBare && !restricted) {
    return 'ci.yml must not run quality on tag pushes; set push.branches to main or tags-ignore v*.*.*'
  }
  return null
}
