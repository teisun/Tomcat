package com.tomcat.controller;

import com.tomcat.controller.request.AuthenticationRequest;
import com.tomcat.controller.response.AuthenticationResponse;
import com.tomcat.service.UserService;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.context.annotation.Lazy;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RestController;


@RestController
public class AuthController {

  @Lazy
  @Autowired
  private UserService userService;


  @PostMapping("/registerOrLogin")
  public ResponseEntity<AuthenticationResponse> registerOrLogin(@RequestBody AuthenticationRequest request) {
    AuthenticationResponse response = userService.registerOrLogin(request);
    if(response != null){
      return ResponseEntity.ok(response);
    }else {
      return ResponseEntity.badRequest().build();
    }

  }  


}