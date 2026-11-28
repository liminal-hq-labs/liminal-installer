@echo off
rem Stands in for HELPER.EXE /quit: records what it was asked and whether the product's files were still there.
if exist "%~dp0HELPER.EXE" (echo %1 files-present> "%TEMP%\li-pre-uninstall-ran.txt") else (echo %1 files-gone> "%TEMP%\li-pre-uninstall-ran.txt")
