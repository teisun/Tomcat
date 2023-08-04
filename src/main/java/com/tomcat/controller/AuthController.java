package com.tomcat.controller;

import com.tomcat.controller.request.AuthenticationRequest;
import com.tomcat.controller.response.AuthenticationResponse;
import com.tomcat.domain.User;
import com.tomcat.domain.UserRepository;
import com.tomcat.service.UserService;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RestController;

import java.util.Optional;

@RestController
public class AuthController {

  @Autowired
  private UserService userService;

  @Autowired
  private UserRepository userRepository;

  @PostMapping("/registerOrLogin")
  public ResponseEntity<AuthenticationResponse> registerOrLogin(@RequestBody AuthenticationRequest request) {
    AuthenticationResponse response;
    User user = userRepository.findByUsername(request.username).get();
    if(user == null){
      response = userService.register(request);
    }else {
      response = userService.login(request);
    }
    userService.register(request);
    return ResponseEntity.ok(response);
  }  


}