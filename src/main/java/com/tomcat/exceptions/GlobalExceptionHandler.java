package com.tomcat.exceptions;

import org.springframework.http.HttpHeaders;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.http.converter.HttpMessageNotReadableException;
import org.springframework.messaging.handler.annotation.support.MethodArgumentNotValidException;
import org.springframework.messaging.handler.annotation.support.MethodArgumentTypeMismatchException;
import org.springframework.web.HttpRequestMethodNotSupportedException;
import org.springframework.web.bind.annotation.ControllerAdvice;
import org.springframework.web.bind.annotation.ExceptionHandler;
import org.springframework.web.context.request.WebRequest;

import java.nio.file.AccessDeniedException;
import java.security.InvalidParameterException;
import java.util.NoSuchElementException;

@ControllerAdvice
public class GlobalExceptionHandler {

  @ExceptionHandler(MethodArgumentNotValidException.class)
  public ResponseEntity<ApiErrorResponse> handleValidationException(MethodArgumentNotValidException ex) {
    ApiErrorResponse response = new ApiErrorResponse.ApiErrorResponseBuilder()
            .withStatus(HttpStatus.BAD_REQUEST)
            .withErrorCode("VALIDATION_ERROR")
            .withMessage(ex.getBindingResult().getAllErrors().get(0).getDefaultMessage())
            .build();

    return ResponseEntity.status(response.getStatus()).body(response);
  }

  @ExceptionHandler(AccessDeniedException.class)
  public ResponseEntity<ApiErrorResponse> handleAccessDeniedException() {
    ApiErrorResponse response = new ApiErrorResponse.ApiErrorResponseBuilder()
            .withStatus(HttpStatus.FORBIDDEN)
            .withErrorCode("ACCESS_DENIED")
            .withMessage("You don't have permission to access this resource")
            .build();

    return ResponseEntity.status(response.getStatus()).body(response);
  }

  @ExceptionHandler(NoSuchElementException.class)
  public ResponseEntity<ApiErrorResponse> handleNoSuchElementException(NoSuchElementException ex) {

    ApiErrorResponse response = new ApiErrorResponse.ApiErrorResponseBuilder()
            .withStatus(HttpStatus.NOT_FOUND)
            .withErrorCode("NOT_FOUND")
            .withMessage(ex.getMessage())
            .build();

    return ResponseEntity.status(response.getStatus()).body(response);
  }


  @ExceptionHandler(BusinessException.class)
  public ResponseEntity<ApiErrorResponse> handleBusinessException(BusinessException ex) {
    ApiErrorResponse response = new ApiErrorResponse.ApiErrorResponseBuilder()
            .withStatus(HttpStatus.INTERNAL_SERVER_ERROR)
            .withErrorCode(ex.getErrorCode())
            .withMessage(ex.getMessage())
            .build();

    return ResponseEntity.status(response.getStatus()).body(response);
  }

  @ExceptionHandler({MethodArgumentTypeMismatchException.class})
  protected ResponseEntity<ApiErrorResponse> handleMethodArgumentNotValid(MethodArgumentNotValidException ex){
    ApiErrorResponse response =new ApiErrorResponse.ApiErrorResponseBuilder()
            .withStatus(HttpStatus.BAD_REQUEST)
            .withErrorCode(HttpStatus.BAD_REQUEST.name())
            .withMessage(ex.getLocalizedMessage()).build();
    return ResponseEntity.status(response.getStatus()).body(response);
  }

  @ExceptionHandler(InvalidParameterException.class)
  public ResponseEntity<ApiErrorResponse> handleInvalidParameter(InvalidParameterException ex) {
    ApiErrorResponse response =new ApiErrorResponse.ApiErrorResponseBuilder()
            .withStatus(HttpStatus.BAD_REQUEST)
            .withErrorCode(HttpStatus.BAD_REQUEST.name())
            .withMessage(ex.getLocalizedMessage()).build();
    return ResponseEntity.status(response.getStatus()).body(response);
  }

  @ExceptionHandler(HttpRequestMethodNotSupportedException.class)
  public ResponseEntity<ApiErrorResponse> handleHttpRequestMethodNotSupportedException(Exception ex) {
    ApiErrorResponse response =new ApiErrorResponse.ApiErrorResponseBuilder()
            .withStatus(HttpStatus.METHOD_NOT_ALLOWED)
            .withErrorCode(HttpStatus.METHOD_NOT_ALLOWED.name())
            .withMessage(ex.getLocalizedMessage()).build();
    return ResponseEntity.status(response.getStatus()).body(response);
  }

  @ExceptionHandler({HttpMessageNotReadableException.class})
  protected ResponseEntity<Object> handleHttpMessageNotReadable(HttpMessageNotReadableException ex, HttpHeaders headers, HttpStatus status, WebRequest request) {
    String error = "Malformed JSON request ";
    ApiErrorResponse response = new ApiErrorResponse.ApiErrorResponseBuilder()
            .withStatus(status)
            .withErrorCode("BAD_DATA")
            .withMessage(ex.getLocalizedMessage())
            .withDetail(error + ex.getMessage()).build();
    return new ResponseEntity<>(response, response.getStatus());
  }


  @ExceptionHandler(Exception.class)
  public ResponseEntity<ApiErrorResponse> handleNotFound(Exception ex) {
    ApiErrorResponse response =new ApiErrorResponse.ApiErrorResponseBuilder()
            .withStatus(HttpStatus.INTERNAL_SERVER_ERROR)
            .withErrorCode(HttpStatus.INTERNAL_SERVER_ERROR.name())
            .withMessage(ex.getLocalizedMessage()).build();
    return ResponseEntity.status(response.getStatus()).body(response);
  }



}