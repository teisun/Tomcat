package com.tomcat.exceptions;



/**
 * 继承自RuntimeException,不强制处理
 *         statusCode - 状态码,如400、500等
 *         errorCode - 错误码,用于区分不同错误
 *         构造函数允许灵活配置状态码和错误码
 *         getters设置状态码和错误码
 */
public class BusinessException extends RuntimeException {


  private String errorCode;

  public BusinessException(String message) {
    super(message);
  }

  public BusinessException(String message, Throwable cause) {
    super(message, cause);
  }


  public String getErrorCode() {
    return errorCode;
  }

  public void setErrorCode(String errorCode) {
    this.errorCode = errorCode;
  }



}