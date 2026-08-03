export function getPlaybookProgress(
  currentStep: number,
  totalSteps: number,
): number {
  if (totalSteps <= 0) return 0;
  return Math.min(Math.max(currentStep, 0), totalSteps) / totalSteps;
}
