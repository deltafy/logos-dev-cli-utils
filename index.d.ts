/* tslint:disable */
/* eslint-disable */

/* auto-generated by NAPI-RS */

export interface PgResponse {
  code: string
  message: string
}
export interface ProcessOutput {
  status: number
  stdout: string
  stderr: string
}
export declare function runNpmScript(script: string): Promise<ProcessOutput>
export declare function testPostgresUrl(url: string): Promise<PgResponse>
export declare function createDatabase(url: string, database: string): Promise<PgResponse>
export declare function testRedisParameters(host: string, username?: string | undefined | null, password?: string | undefined | null): Promise<string>
export declare function fileExists(filePath: string): boolean
export declare function renameDatabase(url: string, database: string, newDatabaseName: string): Promise<PgResponse>
export declare function findNonexistentFiles(paths: Array<string>): Array<string>
export declare function copyFile(source: string, destination: string, createDestIfNotExists?: boolean | undefined | null): void
export declare function envToJsonString(envPath: string): string
export declare function jsonStringToEnv(jsonStr: string, envPath: string): void
