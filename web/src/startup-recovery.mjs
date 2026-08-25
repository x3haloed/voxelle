/**
 * Retry initial native snapshot loading only after the human asks to try again.
 * @template T
 * @param {() => Promise<T>} execute
 * @param {(error: unknown) => Promise<void>} waitForRetry
 * @returns {Promise<T>}
 */
export async function loadInitialSnapshotWithRetry(execute, waitForRetry) {
  while (true) {
    try {
      return await execute();
    } catch (error) {
      await waitForRetry(error);
    }
  }
}
