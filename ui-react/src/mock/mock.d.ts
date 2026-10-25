/** The mock kernel is plain JS (ported verbatim from ui/js/mock.js) so it stays
 *  byte-identical with the old frontend and its Node smoke tests. Only the
 *  public surface is declared here. */

export declare function mockCall(command: string, args?: Record<string, unknown>): Promise<unknown>;
